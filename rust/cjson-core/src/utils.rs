// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! RFC 6901 / 6902 / 7396 utilities matching `cJSON_Utils.c`.

use crate::compare::compare_json;
use crate::dom::{
    add_item_to_array, add_item_to_object, create_array, create_null, create_object, create_string,
    detach_item_from_object, detach_item_via_pointer, duplicate, get_array_item, get_object_item,
    is_array, is_null, is_object, is_string,
};
use crate::store::{key_owned, valuestring_owned, NodeId, Store};
use crate::types::{self, case_insensitive_strcmp, type_kind};

fn compare_strings(a: Option<&[u8]>, b: Option<&[u8]>, case_sensitive: bool) -> i32 {
    match (a, b) {
        (None, _) | (_, None) => 1,
        (Some(x), Some(y)) if core::ptr::eq(x, y) => 0,
        (Some(x), Some(y)) if case_sensitive => match x.cmp(y) {
            core::cmp::Ordering::Less => -1,
            core::cmp::Ordering::Equal => 0,
            core::cmp::Ordering::Greater => 1,
        },
        (Some(_), Some(_)) => case_insensitive_strcmp(a, b),
    }
}

fn compare_pointers(name: Option<&[u8]>, pointer: &[u8], case_sensitive: bool) -> bool {
    let Some(name) = name else {
        return false;
    };
    let mut ni = 0usize;
    let mut pi = 0usize;
    while ni < name.len() && pi < pointer.len() && pointer[pi] != b'/' {
        if pointer[pi] == b'~' {
            let ok = (pi + 1 < pointer.len() && pointer[pi + 1] == b'0' && name[ni] == b'~')
                || (pi + 1 < pointer.len() && pointer[pi + 1] == b'1' && name[ni] == b'/');
            if !ok {
                return false;
            }
            pi += 1;
        } else if (!case_sensitive && crate::types::c_tolower(name[ni]) != crate::types::c_tolower(pointer[pi]))
            || (case_sensitive && name[ni] != pointer[pi])
        {
            return false;
        }
        ni += 1;
        pi += 1;
    }
    let pointer_ended = pi >= pointer.len() || pointer[pi] == b'/';
    let name_ended = ni >= name.len();
    pointer_ended == name_ended
}

fn pointer_encoded_length(s: &[u8]) -> usize {
    let mut len = 0usize;
    for &c in s {
        len += 1;
        if c == b'~' || c == b'/' {
            len += 1;
        }
    }
    len
}

fn encode_string_as_pointer(source: &[u8]) -> Vec<u8> {
    let mut dest = Vec::with_capacity(pointer_encoded_length(source));
    for &c in source {
        match c {
            b'/' => dest.extend_from_slice(b"~1"),
            b'~' => dest.extend_from_slice(b"~0"),
            c => dest.push(c),
        }
    }
    dest
}

/// RFC 6901 decode of `~0` / `~1` (correct encoding; C's in-place decoder is used on a copy).
fn decode_pointer(string: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(string.len());
    let mut i = 0;
    while i < string.len() {
        if string[i] == b'~' && i + 1 < string.len() {
            match string[i + 1] {
                b'0' => {
                    out.push(b'~');
                    i += 2;
                    continue;
                }
                b'1' => {
                    out.push(b'/');
                    i += 2;
                    continue;
                }
                _ => {
                    out.extend_from_slice(&string[i..]);
                    break;
                }
            }
        }
        out.push(string[i]);
        i += 1;
    }
    out
}

fn decode_array_index(pointer: &[u8]) -> Option<usize> {
    if pointer.first() == Some(&b'0')
        && pointer.get(1).is_some()
        && pointer[1] != b'/'
        && pointer[1] != 0
    {
        // leading zero not permitted unless the whole token is "0"
        if pointer[1].is_ascii_digit() {
            return None;
        }
    }
    let mut parsed = 0usize;
    let mut pos = 0usize;
    while pos < pointer.len() && pointer[pos].is_ascii_digit() {
        parsed = parsed.saturating_mul(10).saturating_add((pointer[pos] - b'0') as usize);
        pos += 1;
    }
    if pos < pointer.len() && pointer[pos] != 0 && pointer[pos] != b'/' {
        return None;
    }
    Some(parsed)
}

pub fn get_item_from_pointer<S: Store>(
    store: &S,
    object: Option<NodeId>,
    pointer: &[u8],
    case_sensitive: bool,
) -> Option<NodeId> {
    let mut current = object;
    let mut p = pointer;
    while p.first() == Some(&b'/') && current.is_some() {
        p = &p[1..];
        let cur = current?;
        if is_array(store, Some(cur)) {
            let index = decode_array_index(p)?;
            current = get_array_item(store, Some(cur), index as i32);
        } else if is_object(store, Some(cur)) {
            let mut el = store.child(cur);
            while let Some(e) = el {
                if compare_pointers(store.key_bytes(e), p, case_sensitive) {
                    current = Some(e);
                    break;
                }
                el = store.next(e);
                current = el;
            }
            if el.is_none() {
                current = None;
            }
        } else {
            return None;
        }
        while !p.is_empty() && p[0] != b'/' {
            p = &p[1..];
        }
    }
    current
}

pub fn find_pointer_from_object_to<S: Store>(
    store: &S,
    object: NodeId,
    target: NodeId,
) -> Option<Vec<u8>> {
    if object == target {
        return Some(Vec::new());
    }
    let mut child = store.child(object);
    let mut child_index = 0usize;
    while let Some(c) = child {
        if let Some(sub) = find_pointer_from_object_to(store, c, target) {
            if is_array(store, Some(object)) {
                let mut full = format!("/{child_index}").into_bytes();
                full.extend_from_slice(&sub);
                return Some(full);
            }
            if is_object(store, Some(object)) {
                let key = store.key_bytes(c).unwrap_or(b"");
                let mut full = vec![b'/'];
                full.extend_from_slice(&encode_string_as_pointer(key));
                full.extend_from_slice(&sub);
                return Some(full);
            }
            return None;
        }
        child = store.next(c);
        child_index += 1;
    }
    None
}

fn collect_list<S: Store>(store: &S, head: Option<NodeId>) -> Vec<NodeId> {
    let mut v = Vec::new();
    let mut c = head;
    while let Some(n) = c {
        v.push(n);
        c = store.next(n);
    }
    v
}

fn relink<S: Store>(store: &mut S, nodes: &[NodeId]) {
    for (i, &n) in nodes.iter().enumerate() {
        store.set_prev(n, if i == 0 { None } else { Some(nodes[i - 1]) });
        store.set_next(n, if i + 1 == nodes.len() { None } else { Some(nodes[i + 1]) });
    }
}

pub fn sort_list<S: Store>(
    store: &mut S,
    list: Option<NodeId>,
    case_sensitive: bool,
) -> Option<NodeId> {
    let Some(head) = list else {
        return None;
    };
    if store.next(head).is_none() {
        return Some(head);
    }
    // already sorted?
    let mut current = Some(head);
    let mut sorted = true;
    while let Some(c) = current {
        if let Some(n) = store.next(c) {
            if compare_strings(store.key_bytes(c), store.key_bytes(n), case_sensitive) >= 0 {
                sorted = false;
                break;
            }
            current = Some(n);
        } else {
            break;
        }
    }
    if sorted {
        return Some(head);
    }
    let mut nodes = collect_list(store, Some(head));
    nodes.sort_by(|&a, &b| {
        let cmp = compare_strings(store.key_bytes(a), store.key_bytes(b), case_sensitive);
        if cmp < 0 {
            core::cmp::Ordering::Less
        } else if cmp > 0 {
            core::cmp::Ordering::Greater
        } else {
            core::cmp::Ordering::Equal
        }
    });
    relink(store, &nodes);
    nodes.first().copied()
}

pub fn sort_object<S: Store>(store: &mut S, object: NodeId, case_sensitive: bool) {
    let sorted = sort_list(store, store.child(object), case_sensitive);
    store.set_child(object, sorted);
}

fn detach_item_from_array_usize<S: Store>(
    store: &mut S,
    array: NodeId,
    which: usize,
) -> Option<NodeId> {
    let mut c = store.child(array);
    let mut left = which;
    while let Some(n) = c {
        if left == 0 {
            break;
        }
        c = store.next(n);
        left -= 1;
    }
    let c = c?;
    detach_item_via_pointer(store, array, c)
}

fn insert_item_in_array_usize<S: Store>(
    store: &mut S,
    array: NodeId,
    which: usize,
    newitem: NodeId,
) -> bool {
    let mut child = store.child(array);
    let mut left = which;
    while child.is_some() && left > 0 {
        child = store.next(child.unwrap());
        left -= 1;
    }
    if left > 0 {
        return false;
    }
    if child.is_none() {
        return add_item_to_array(store, array, newitem);
    }
    let child = child.unwrap();
    store.set_next(newitem, Some(child));
    store.set_prev(newitem, store.prev(child));
    store.set_prev(child, Some(newitem));
    if store.child(array) == Some(child) {
        store.set_child(array, Some(newitem));
    } else if let Some(p) = store.prev(newitem) {
        store.set_next(p, Some(newitem));
    }
    true
}

fn detach_path<S: Store>(
    store: &mut S,
    object: NodeId,
    path: &[u8],
    case_sensitive: bool,
) -> Option<NodeId> {
    let slash = path.iter().rposition(|&c| c == b'/')?;
    let parent_path = &path[..slash];
    let child_enc = &path[slash + 1..];
    let child = decode_pointer(child_enc);
    let parent = get_item_from_pointer(store, Some(object), parent_path, case_sensitive)?;
    if is_array(store, Some(parent)) {
        let index = decode_array_index(&child)?;
        detach_item_from_array_usize(store, parent, index)
    } else if is_object(store, Some(parent)) {
        detach_item_from_object(store, parent, &child, case_sensitive)
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PatchOp {
    Invalid,
    Add,
    Remove,
    Replace,
    Move,
    Copy,
    Test,
}

fn decode_patch_operation<S: Store>(store: &S, patch: NodeId, case_sensitive: bool) -> PatchOp {
    let Some(op) = get_object_item(store, Some(patch), Some(b"op"), case_sensitive) else {
        return PatchOp::Invalid;
    };
    if !is_string(store, Some(op)) {
        return PatchOp::Invalid;
    }
    match store.valuestring_bytes(op) {
        Some(b"add") => PatchOp::Add,
        Some(b"remove") => PatchOp::Remove,
        Some(b"replace") => PatchOp::Replace,
        Some(b"move") => PatchOp::Move,
        Some(b"copy") => PatchOp::Copy,
        Some(b"test") => PatchOp::Test,
        _ => PatchOp::Invalid,
    }
}

fn apply_patch<S: Store>(store: &mut S, object: NodeId, patch: NodeId, case_sensitive: bool) -> i32 {
    let Some(path_item) = get_object_item(store, Some(patch), Some(b"path"), case_sensitive) else {
        return 2;
    };
    if !is_string(store, Some(path_item)) {
        return 2;
    }
    let path = valuestring_owned(store, path_item).unwrap_or_default();
    let opcode = decode_patch_operation(store, patch, case_sensitive);
    if opcode == PatchOp::Invalid {
        return 3;
    }
    if opcode == PatchOp::Test {
        let target = get_item_from_pointer(store, Some(object), &path, case_sensitive);
        let value = get_object_item(store, Some(patch), Some(b"value"), case_sensitive);
        return if compare_json(store, target, value, case_sensitive) {
            0
        } else {
            1
        };
    }
    if path.is_empty() {
        if opcode == PatchOp::Remove {
            store.set_ty(object, types::INVALID);
            store.set_child(object, None);
            if let Some(s) = store.valuestring_id(object) {
                store.set_valuestring_id(object, None);
                store.free_str(s);
            }
            if let Some(s) = store.key_id(object) {
                store.set_key_id(object, None);
                store.free_str(s);
            }
            return 0;
        }
        if opcode == PatchOp::Replace || opcode == PatchOp::Add {
            let Some(value) = get_object_item(store, Some(patch), Some(b"value"), case_sensitive)
            else {
                return 7;
            };
            let Some(dup) = duplicate(store, value, true) else {
                return 8;
            };
            store.steal_into(object, dup);
            if let Some(s) = store.key_id(object) {
                store.set_key_id(object, None);
                store.free_str(s);
            }
            return 0;
        }
    }
    if opcode == PatchOp::Remove || opcode == PatchOp::Replace {
        let Some(old) = detach_path(store, object, &path, case_sensitive) else {
            return 13;
        };
        store.delete(Some(old));
        if opcode == PatchOp::Remove {
            return 0;
        }
    }
    let value = if opcode == PatchOp::Move || opcode == PatchOp::Copy {
        let Some(from) = get_object_item(store, Some(patch), Some(b"from"), case_sensitive) else {
            return 4;
        };
        if !is_string(store, Some(from)) {
            return 4;
        }
        let from_path = valuestring_owned(store, from).unwrap_or_default();
        let v = if opcode == PatchOp::Move {
            detach_path(store, object, &from_path, case_sensitive)
        } else {
            get_item_from_pointer(store, Some(object), &from_path, case_sensitive)
        };
        let Some(v) = v else {
            return 5;
        };
        if opcode == PatchOp::Copy {
            match duplicate(store, v, true) {
                Some(d) => d,
                None => return 6,
            }
        } else {
            v
        }
    } else {
        let Some(v) = get_object_item(store, Some(patch), Some(b"value"), case_sensitive) else {
            return 7;
        };
        match duplicate(store, v, true) {
            Some(d) => d,
            None => return 8,
        }
    };

    let slash = path.iter().rposition(|&c| c == b'/');
    let (parent_path, child_enc) = match slash {
        Some(i) => (&path[..i], &path[i + 1..]),
        None => {
            store.delete(Some(value));
            return 9;
        }
    };
    let child = decode_pointer(child_enc);
    let Some(parent) = get_item_from_pointer(store, Some(object), parent_path, case_sensitive)
    else {
        store.delete(Some(value));
        return 9;
    };
    if is_array(store, Some(parent)) {
        if child == b"-" {
            add_item_to_array(store, parent, value);
            return 0;
        }
        let Some(index) = decode_array_index(&child) else {
            store.delete(Some(value));
            return 11;
        };
        if !insert_item_in_array_usize(store, parent, index, value) {
            store.delete(Some(value));
            return 10;
        }
        return 0;
    }
    if is_object(store, Some(parent)) {
        if let Some(old) = detach_item_from_object(store, parent, &child, case_sensitive) {
            store.delete(Some(old));
        }
        add_item_to_object(store, parent, &child, value, false);
        let _ = value;
        return 0;
    }
    store.delete(Some(value));
    9
}

pub fn apply_patches<S: Store>(
    store: &mut S,
    object: NodeId,
    patches: NodeId,
    case_sensitive: bool,
) -> i32 {
    if !is_array(store, Some(patches)) {
        return 1;
    }
    let mut current = store.child(patches);
    while let Some(p) = current {
        let status = apply_patch(store, object, p, case_sensitive);
        if status != 0 {
            return status;
        }
        current = store.next(p);
    }
    0
}

fn compose_patch<S: Store>(
    store: &mut S,
    patches: NodeId,
    operation: &[u8],
    path: &[u8],
    suffix: Option<&[u8]>,
    value: Option<NodeId>,
) {
    let Some(patch) = create_object(store) else {
        return;
    };
    if let Some(op) = create_string(store, operation) {
        add_item_to_object(store, patch, b"op", op, false);
    }
    let full_path = if let Some(suf) = suffix {
        let mut p = path.to_vec();
        p.push(b'/');
        p.extend_from_slice(&encode_string_as_pointer(suf));
        p
    } else {
        path.to_vec()
    };
    if let Some(ps) = create_string(store, &full_path) {
        add_item_to_object(store, patch, b"path", ps, false);
    }
    if let Some(v) = value {
        if let Some(d) = duplicate(store, v, true) {
            add_item_to_object(store, patch, b"value", d, false);
        }
    }
    add_item_to_array(store, patches, patch);
}

pub fn add_patch_to_array<S: Store>(
    store: &mut S,
    array: NodeId,
    operation: &[u8],
    path: &[u8],
    value: Option<NodeId>,
) {
    compose_patch(store, array, operation, path, None, value);
}

fn create_patches<S: Store>(
    store: &mut S,
    patches: NodeId,
    path: &[u8],
    from: NodeId,
    to: NodeId,
    case_sensitive: bool,
) {
    if type_kind(store.ty(from)) != type_kind(store.ty(to)) {
        compose_patch(store, patches, b"replace", path, None, Some(to));
        return;
    }
    match type_kind(store.ty(from)) {
        types::NUMBER => {
            if store.valueint(from) != store.valueint(to)
                || !crate::number::compare_double(store.valuedouble(from), store.valuedouble(to))
            {
                compose_patch(store, patches, b"replace", path, None, Some(to));
            }
        }
        types::STRING => {
            if store.valuestring_bytes(from) != store.valuestring_bytes(to) {
                compose_patch(store, patches, b"replace", path, None, Some(to));
            }
        }
        types::ARRAY => {
            let mut from_child = store.child(from);
            let mut to_child = store.child(to);
            let mut index = 0usize;
            while let (Some(fc), Some(tc)) = (from_child, to_child) {
                let new_path = format!("{}/{index}", String::from_utf8_lossy(path)).into_bytes();
                create_patches(store, patches, &new_path, fc, tc, case_sensitive);
                from_child = store.next(fc);
                to_child = store.next(tc);
                index += 1;
            }
            while let Some(fc) = from_child {
                let idx = format!("{index}");
                compose_patch(store, patches, b"remove", path, Some(idx.as_bytes()), None);
                from_child = store.next(fc);
            }
            while let Some(tc) = to_child {
                compose_patch(store, patches, b"add", path, Some(b"-"), Some(tc));
                to_child = store.next(tc);
            }
        }
        types::OBJECT => {
            sort_object(store, from, case_sensitive);
            sort_object(store, to, case_sensitive);
            let mut from_child = store.child(from);
            let mut to_child = store.child(to);
            while from_child.is_some() || to_child.is_some() {
                let diff = match (from_child, to_child) {
                    (None, Some(_)) => 1,
                    (Some(_), None) => -1,
                    (Some(f), Some(t)) => {
                        compare_strings(store.key_bytes(f), store.key_bytes(t), case_sensitive)
                    }
                    _ => 0,
                };
                if diff == 0 {
                    let f = from_child.unwrap();
                    let t = to_child.unwrap();
                    let key = key_owned(store, f).unwrap_or_default();
                    let mut new_path = path.to_vec();
                    new_path.push(b'/');
                    new_path.extend_from_slice(&encode_string_as_pointer(&key));
                    create_patches(store, patches, &new_path, f, t, case_sensitive);
                    from_child = store.next(f);
                    to_child = store.next(t);
                } else if diff < 0 {
                    let f = from_child.unwrap();
                    let key = key_owned(store, f).unwrap_or_default();
                    compose_patch(store, patches, b"remove", path, Some(&key), None);
                    from_child = store.next(f);
                } else {
                    let t = to_child.unwrap();
                    let key = key_owned(store, t).unwrap_or_default();
                    compose_patch(store, patches, b"add", path, Some(&key), Some(t));
                    to_child = store.next(t);
                }
            }
        }
        _ => {}
    }
}

pub fn generate_patches<S: Store>(
    store: &mut S,
    from: NodeId,
    to: NodeId,
    case_sensitive: bool,
) -> Option<NodeId> {
    let patches = create_array(store)?;
    create_patches(store, patches, b"", from, to, case_sensitive);
    Some(patches)
}

pub fn merge_patch<S: Store>(
    store: &mut S,
    target: Option<NodeId>,
    patch: Option<NodeId>,
    case_sensitive: bool,
) -> Option<NodeId> {
    let patch = patch?;
    if !is_object(store, Some(patch)) {
        let dup = duplicate(store, patch, true);
        if let Some(t) = target {
            store.delete(Some(t));
        }
        return dup;
    }
    let target = if !is_object(store, target) {
        if let Some(t) = target {
            store.delete(Some(t));
        }
        create_object(store)?
    } else {
        target.unwrap()
    };
    let mut patch_child = store.child(patch);
    while let Some(pc) = patch_child {
        let key = key_owned(store, pc).unwrap_or_default();
        if is_null(store, Some(pc)) {
            if let Some(old) = detach_item_from_object(store, target, &key, case_sensitive) {
                store.delete(Some(old));
            }
        } else {
            let replace_me = detach_item_from_object(store, target, &key, case_sensitive);
            let Some(replacement) = merge_patch(store, replace_me, Some(pc), case_sensitive) else {
                store.delete(Some(target));
                return None;
            };
            add_item_to_object(store, target, &key, replacement, false);
        }
        patch_child = store.next(pc);
    }
    Some(target)
}

pub fn generate_merge_patch<S: Store>(
    store: &mut S,
    from: Option<NodeId>,
    to: Option<NodeId>,
    case_sensitive: bool,
) -> Option<NodeId> {
    let Some(to) = to else {
        return create_null(store);
    };
    let Some(from) = from else {
        return duplicate(store, to, true);
    };
    if !is_object(store, Some(to)) || !is_object(store, Some(from)) {
        return duplicate(store, to, true);
    }
    sort_object(store, from, case_sensitive);
    sort_object(store, to, case_sensitive);
    let patch = create_object(store)?;
    let mut from_child = store.child(from);
    let mut to_child = store.child(to);
    while from_child.is_some() || to_child.is_some() {
        // C always uses strcmp for this walk, even in the case-insensitive function.
        let diff = match (from_child, to_child) {
            (Some(f), Some(t)) => {
                let a = store.key_bytes(f).unwrap_or(b"");
                let b = store.key_bytes(t).unwrap_or(b"");
                match a.cmp(b) {
                    core::cmp::Ordering::Less => -1,
                    core::cmp::Ordering::Equal => 0,
                    core::cmp::Ordering::Greater => 1,
                }
            }
            (Some(_), None) => -1,
            (None, Some(_)) => 1,
            _ => 0,
        };
        if diff < 0 {
            let f = from_child.unwrap();
            let key = key_owned(store, f).unwrap_or_default();
            if let Some(n) = create_null(store) {
                add_item_to_object(store, patch, &key, n, false);
            }
            from_child = store.next(f);
        } else if diff > 0 {
            let t = to_child.unwrap();
            let key = key_owned(store, t).unwrap_or_default();
            if let Some(d) = duplicate(store, t, true) {
                add_item_to_object(store, patch, &key, d, false);
            }
            to_child = store.next(t);
        } else {
            let f = from_child.unwrap();
            let t = to_child.unwrap();
            if !compare_json(store, Some(f), Some(t), case_sensitive) {
                let key = key_owned(store, t).unwrap_or_default();
                // C CaseSensitive variant still recurses through the non-CS public function.
                if let Some(sub) = generate_merge_patch(store, Some(f), Some(t), false) {
                    add_item_to_object(store, patch, &key, sub, false);
                }
            }
            from_child = store.next(f);
            to_child = store.next(t);
        }
    }
    if store.child(patch).is_none() {
        store.delete(Some(patch));
        return None;
    }
    Some(patch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, ParseOpts};
    use crate::store::SafeStore;

    #[test]
    fn pointer_and_sort() {
        let mut s = SafeStore::new();
        let root = parse(&mut s, b"{\"b\":1,\"a\":2}", ParseOpts::default()).unwrap();
        sort_object(&mut s, root, true);
        let first = s.child(root).unwrap();
        assert_eq!(s.key_bytes(first), Some(&b"a"[..]));
        let p = get_item_from_pointer(&s, Some(root), b"/a", true).unwrap();
        assert_eq!(s.valuedouble(p), 2.0);
    }
}
