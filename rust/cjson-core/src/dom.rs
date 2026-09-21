// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Create / mutate / lookup / duplicate — the public cJSON DOM API.

use crate::number::{saturate_int, set_number_fields};
use crate::store::{key_owned, NodeId, Store};
use crate::types::{self, is_reference, string_is_const, type_kind, CIRCULAR_LIMIT};

pub fn create_null<S: Store>(store: &mut S) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::NULL);
    Some(n)
}
pub fn create_true<S: Store>(store: &mut S) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::TRUE);
    Some(n)
}
pub fn create_false<S: Store>(store: &mut S) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::FALSE);
    Some(n)
}
pub fn create_bool<S: Store>(store: &mut S, v: bool) -> Option<NodeId> {
    if v {
        create_true(store)
    } else {
        create_false(store)
    }
}
pub fn create_number<S: Store>(store: &mut S, num: f64) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::NUMBER);
    set_number_fields(store, n, num);
    Some(n)
}
pub fn create_string<S: Store>(store: &mut S, s: &[u8]) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::STRING);
    match store.alloc_str(s) {
        Some(sid) => {
            store.set_valuestring_id(n, Some(sid));
            Some(n)
        }
        None => {
            store.delete(Some(n));
            None
        }
    }
}
pub fn create_raw<S: Store>(store: &mut S, s: &[u8]) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::RAW);
    match store.alloc_str(s) {
        Some(sid) => {
            store.set_valuestring_id(n, Some(sid));
            Some(n)
        }
        None => {
            store.delete(Some(n));
            None
        }
    }
}
pub fn create_array<S: Store>(store: &mut S) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::ARRAY);
    Some(n)
}
pub fn create_object<S: Store>(store: &mut S) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::OBJECT);
    Some(n)
}

pub fn create_string_reference<S: Store>(store: &mut S, s: &[u8]) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::STRING | types::IS_REFERENCE);
    let sid = store.borrow_str(s)?;
    store.set_valuestring_id(n, Some(sid));
    Some(n)
}

pub fn create_object_reference<S: Store>(store: &mut S, child: Option<NodeId>) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::OBJECT | types::IS_REFERENCE);
    store.set_child(n, child);
    Some(n)
}

pub fn create_array_reference<S: Store>(store: &mut S, child: Option<NodeId>) -> Option<NodeId> {
    let n = store.alloc_node()?;
    store.set_ty(n, types::ARRAY | types::IS_REFERENCE);
    store.set_child(n, child);
    Some(n)
}

pub fn suffix_object<S: Store>(store: &mut S, prev: NodeId, item: NodeId) {
    store.set_next(prev, Some(item));
    store.set_prev(item, Some(prev));
}

pub fn add_item_to_array<S: Store>(store: &mut S, array: NodeId, item: NodeId) -> bool {
    if array == item {
        return false;
    }
    match store.child(array) {
        None => {
            store.set_child(array, Some(item));
            store.set_prev(item, Some(item));
            store.set_next(item, None);
            true
        }
        Some(child) => {
            if let Some(tail) = store.prev(child) {
                suffix_object(store, tail, item);
                store.set_prev(child, Some(item));
            }
            true
        }
    }
}

pub fn add_item_to_object<S: Store>(
    store: &mut S,
    object: NodeId,
    key: &[u8],
    item: NodeId,
    constant_key: bool,
) -> bool {
    if object == item {
        return false;
    }
    let (new_key, new_type) = if constant_key {
        let Some(sid) = store.borrow_str(key) else {
            return false;
        };
        (sid, store.ty(item) | types::STRING_IS_CONST)
    } else {
        let Some(sid) = store.alloc_str(key) else {
            return false;
        };
        (sid, store.ty(item) & !types::STRING_IS_CONST)
    };
    if !string_is_const(store.ty(item)) {
        if let Some(old) = store.key_id(item) {
            store.set_key_id(item, None);
            store.free_str(old);
        }
    }
    store.set_key_id(item, Some(new_key));
    store.set_ty(item, new_type);
    add_item_to_array(store, object, item)
}

pub fn create_reference<S: Store>(store: &mut S, item: NodeId) -> Option<NodeId> {
    let r = store.alloc_node()?;
    store.copy_fields(r, item);
    store.set_key_id(r, None);
    store.set_ty(r, store.ty(r) | types::IS_REFERENCE);
    store.set_next(r, None);
    store.set_prev(r, None);
    Some(r)
}

pub fn get_array_size<S: Store>(store: &S, array: Option<NodeId>) -> i32 {
    let Some(array) = array else {
        return 0;
    };
    let mut size: usize = 0;
    let mut child = store.child(array);
    while let Some(c) = child {
        size += 1;
        child = store.next(c);
    }
    size as i32
}

pub fn get_array_item<S: Store>(store: &S, array: Option<NodeId>, index: i32) -> Option<NodeId> {
    if index < 0 {
        return None;
    }
    let array = array?;
    let mut child = store.child(array);
    let mut i = index as usize;
    while child.is_some() && i > 0 {
        child = store.next(child.unwrap());
        i -= 1;
    }
    child
}

pub fn get_object_item<S: Store>(
    store: &S,
    object: Option<NodeId>,
    name: Option<&[u8]>,
    case_sensitive: bool,
) -> Option<NodeId> {
    let object = object?;
    let name = name?;
    let mut cur = store.child(object);
    if case_sensitive {
        while let Some(el) = cur {
            match store.key_bytes(el) {
                Some(k) if k == name => break,
                Some(_) => cur = store.next(el),
                None => break,
            }
        }
    } else {
        while let Some(el) = cur {
            if crate::types::case_insensitive_strcmp(Some(name), store.key_bytes(el)) == 0 {
                break;
            }
            cur = store.next(el);
        }
    }
    match cur {
        Some(el) if store.key_bytes(el).is_some() => Some(el),
        _ => None,
    }
}

pub fn detach_item_via_pointer<S: Store>(
    store: &mut S,
    parent: NodeId,
    item: NodeId,
) -> Option<NodeId> {
    let child = store.child(parent);
    if Some(item) != child && store.prev(item).is_none() {
        return None;
    }
    if Some(item) != child {
        if let Some(prev) = store.prev(item) {
            store.set_next(prev, store.next(item));
        }
    }
    if let Some(next) = store.next(item) {
        store.set_prev(next, store.prev(item));
    }
    if Some(item) == child {
        store.set_child(parent, store.next(item));
    } else if store.next(item).is_none() {
        if let Some(c) = store.child(parent) {
            store.set_prev(c, store.prev(item));
        }
    }
    store.set_prev(item, None);
    store.set_next(item, None);
    Some(item)
}

pub fn detach_item_from_array<S: Store>(
    store: &mut S,
    array: NodeId,
    which: i32,
) -> Option<NodeId> {
    let item = get_array_item(store, Some(array), which)?;
    detach_item_via_pointer(store, array, item)
}

pub fn detach_item_from_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    case_sensitive: bool,
) -> Option<NodeId> {
    let item = get_object_item(store, Some(object), Some(name), case_sensitive)?;
    detach_item_via_pointer(store, object, item)
}

pub fn insert_item_in_array<S: Store>(
    store: &mut S,
    array: NodeId,
    which: i32,
    newitem: NodeId,
) -> bool {
    if which < 0 {
        return false;
    }
    let Some(after) = get_array_item(store, Some(array), which) else {
        return add_item_to_array(store, array, newitem);
    };
    if Some(after) != store.child(array) && store.prev(after).is_none() {
        return false;
    }
    store.set_next(newitem, Some(after));
    store.set_prev(newitem, store.prev(after));
    store.set_prev(after, Some(newitem));
    if Some(after) == store.child(array) {
        store.set_child(array, Some(newitem));
    } else if let Some(p) = store.prev(newitem) {
        store.set_next(p, Some(newitem));
    }
    true
}

pub fn replace_item_via_pointer<S: Store>(
    store: &mut S,
    parent: NodeId,
    item: NodeId,
    replacement: NodeId,
) -> bool {
    if store.child(parent).is_none() {
        return false;
    }
    if replacement == item {
        return true;
    }
    store.set_next(replacement, store.next(item));
    store.set_prev(replacement, store.prev(item));
    if let Some(next) = store.next(replacement) {
        store.set_prev(next, Some(replacement));
    }
    if store.child(parent) == Some(item) {
        if store.prev(item) == Some(item) {
            store.set_prev(replacement, Some(replacement));
        }
        store.set_child(parent, Some(replacement));
    } else {
        if let Some(prev) = store.prev(replacement) {
            store.set_next(prev, Some(replacement));
        }
        if store.next(replacement).is_none() {
            if let Some(c) = store.child(parent) {
                store.set_prev(c, Some(replacement));
            }
        }
    }
    store.set_next(item, None);
    store.set_prev(item, None);
    store.delete(Some(item));
    true
}

pub fn replace_item_in_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    replacement: NodeId,
    case_sensitive: bool,
) -> bool {
    if !string_is_const(store.ty(replacement)) {
        if let Some(old) = store.key_id(replacement) {
            store.set_key_id(replacement, None);
            store.free_str(old);
        }
    }
    let Some(sid) = store.alloc_str(name) else {
        return false;
    };
    store.set_key_id(replacement, Some(sid));
    store.set_ty(replacement, store.ty(replacement) & !types::STRING_IS_CONST);
    let Some(item) = get_object_item(store, Some(object), Some(name), case_sensitive) else {
        return false;
    };
    replace_item_via_pointer(store, object, item, replacement)
}

pub fn set_valuestring<S: Store>(store: &mut S, object: NodeId, valuestring: &[u8]) -> bool {
    let ty = store.ty(object);
    if type_kind(ty) != types::STRING || is_reference(ty) {
        return false;
    }
    let Some(old_id) = store.valuestring_id(object) else {
        return false;
    };
    let old = store.valuestring_bytes(object).map(|b| b.to_vec());
    let Some(old) = old else {
        return false;
    };
    if valuestring.len() <= old.len() {
        // C rejects overlapping pointers; SafeStore copies so overlap is N/A.
        store.free_str(old_id);
        let Some(new_id) = store.alloc_str(valuestring) else {
            return false;
        };
        store.set_valuestring_id(object, Some(new_id));
        return true;
    }
    let Some(new_id) = store.alloc_str(valuestring) else {
        return false;
    };
    store.set_valuestring_id(object, Some(new_id));
    store.free_str(old_id);
    true
}

pub fn duplicate<S: Store>(store: &mut S, item: NodeId, recurse: bool) -> Option<NodeId> {
    duplicate_rec(store, item, 0, recurse)
}

fn duplicate_rec<S: Store>(
    store: &mut S,
    item: NodeId,
    depth: usize,
    recurse: bool,
) -> Option<NodeId> {
    let newitem = store.alloc_node()?;
    store.set_ty(newitem, store.ty(item) & !types::IS_REFERENCE);
    store.set_valueint(newitem, store.valueint(item));
    store.set_valuedouble(newitem, store.valuedouble(item));
    if let Some(vs) = store.valuestring_bytes(item).map(|b| b.to_vec()) {
        match store.alloc_str(&vs) {
            Some(sid) => store.set_valuestring_id(newitem, Some(sid)),
            None => {
                store.delete(Some(newitem));
                return None;
            }
        }
    }
    if let Some(key) = key_owned(store, item) {
        if string_is_const(store.ty(item)) {
            if let Some(sid) = store.borrow_str(&key) {
                store.set_key_id(newitem, Some(sid));
            } else {
                store.delete(Some(newitem));
                return None;
            }
        } else {
            match store.alloc_str(&key) {
                Some(sid) => store.set_key_id(newitem, Some(sid)),
                None => {
                    store.delete(Some(newitem));
                    return None;
                }
            }
        }
    }
    if !recurse {
        return Some(newitem);
    }
    let mut child = store.child(item);
    let mut next: Option<NodeId> = None;
    let mut newchild = None;
    while let Some(c) = child {
        if depth >= CIRCULAR_LIMIT {
            store.delete(Some(newitem));
            return None;
        }
        let nc = match duplicate_rec(store, c, depth + 1, true) {
            Some(n) => n,
            None => {
                store.delete(Some(newitem));
                return None;
            }
        };
        newchild = Some(nc);
        if let Some(prev) = next {
            store.set_next(prev, Some(nc));
            store.set_prev(nc, Some(prev));
            next = Some(nc);
        } else {
            store.set_child(newitem, Some(nc));
            next = Some(nc);
        }
        child = store.next(c);
    }
    if let (Some(_), Some(nc)) = (store.child(newitem), newchild) {
        if let Some(ch) = store.child(newitem) {
            store.set_prev(ch, Some(nc));
        }
    }
    Some(newitem)
}

pub fn add_null_to_object<S: Store>(store: &mut S, object: NodeId, name: &[u8]) -> Option<NodeId> {
    let n = create_null(store)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_true_to_object<S: Store>(store: &mut S, object: NodeId, name: &[u8]) -> Option<NodeId> {
    let n = create_true(store)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_false_to_object<S: Store>(store: &mut S, object: NodeId, name: &[u8]) -> Option<NodeId> {
    let n = create_false(store)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_bool_to_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    v: bool,
) -> Option<NodeId> {
    let n = create_bool(store, v)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_number_to_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    num: f64,
) -> Option<NodeId> {
    let n = create_number(store, num)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_string_to_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    s: &[u8],
) -> Option<NodeId> {
    let n = create_string(store, s)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_raw_to_object<S: Store>(
    store: &mut S,
    object: NodeId,
    name: &[u8],
    s: &[u8],
) -> Option<NodeId> {
    let n = create_raw(store, s)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_object_to_object<S: Store>(store: &mut S, object: NodeId, name: &[u8]) -> Option<NodeId> {
    let n = create_object(store)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}
pub fn add_array_to_object<S: Store>(store: &mut S, object: NodeId, name: &[u8]) -> Option<NodeId> {
    let n = create_array(store)?;
    if add_item_to_object(store, object, name, n, false) {
        Some(n)
    } else {
        store.delete(Some(n));
        None
    }
}

pub fn create_int_array<S: Store>(store: &mut S, numbers: &[i32]) -> Option<NodeId> {
    let a = create_array(store)?;
    let mut prev = None;
    let mut last = None;
    for (i, &num) in numbers.iter().enumerate() {
        let n = match create_number(store, num as f64) {
            Some(n) => n,
            None => {
                store.delete(Some(a));
                return None;
            }
        };
        if i == 0 {
            store.set_child(a, Some(n));
        } else if let Some(p) = prev {
            suffix_object(store, p, n);
        }
        prev = Some(n);
        last = Some(n);
    }
    if let (Some(c), Some(n)) = (store.child(a), last) {
        store.set_prev(c, Some(n));
    }
    Some(a)
}

pub fn create_double_array<S: Store>(store: &mut S, numbers: &[f64]) -> Option<NodeId> {
    let a = create_array(store)?;
    let mut prev = None;
    let mut last = None;
    for (i, &num) in numbers.iter().enumerate() {
        let n = match create_number(store, num) {
            Some(n) => n,
            None => {
                store.delete(Some(a));
                return None;
            }
        };
        if i == 0 {
            store.set_child(a, Some(n));
        } else if let Some(p) = prev {
            suffix_object(store, p, n);
        }
        prev = Some(n);
        last = Some(n);
    }
    if let (Some(c), Some(n)) = (store.child(a), last) {
        store.set_prev(c, Some(n));
    }
    Some(a)
}

pub fn create_string_array<S: Store>(store: &mut S, strings: &[&[u8]]) -> Option<NodeId> {
    let a = create_array(store)?;
    let mut prev = None;
    let mut last = None;
    for (i, s) in strings.iter().enumerate() {
        let n = match create_string(store, s) {
            Some(n) => n,
            None => {
                store.delete(Some(a));
                return None;
            }
        };
        if i == 0 {
            store.set_child(a, Some(n));
        } else if let Some(p) = prev {
            suffix_object(store, p, n);
        }
        prev = Some(n);
        last = Some(n);
    }
    if let (Some(c), Some(n)) = (store.child(a), last) {
        store.set_prev(c, Some(n));
    }
    Some(a)
}

pub fn is_invalid<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::INVALID)
        .unwrap_or(false)
}
pub fn is_false<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::FALSE)
        .unwrap_or(false)
}
pub fn is_true<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::TRUE)
        .unwrap_or(false)
}
pub fn is_bool<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| store.ty(i) & (types::TRUE | types::FALSE) != 0)
        .unwrap_or(false)
}
pub fn is_null<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::NULL)
        .unwrap_or(false)
}
pub fn is_number<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::NUMBER)
        .unwrap_or(false)
}
pub fn is_string<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::STRING)
        .unwrap_or(false)
}
pub fn is_array<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::ARRAY)
        .unwrap_or(false)
}
pub fn is_object<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::OBJECT)
        .unwrap_or(false)
}
pub fn is_raw<S: Store>(store: &S, item: Option<NodeId>) -> bool {
    item.map(|i| type_kind(store.ty(i)) == types::RAW)
        .unwrap_or(false)
}

pub fn set_number_helper<S: Store>(store: &mut S, object: NodeId, number: f64) -> f64 {
    store.set_valueint(object, saturate_int(number));
    store.set_valuedouble(object, number);
    number
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SafeStore;

    #[test]
    fn add_and_get() {
        let mut s = SafeStore::new();
        let obj = create_object(&mut s).unwrap();
        add_number_to_object(&mut s, obj, b"n", 3.0).unwrap();
        let item = get_object_item(&s, Some(obj), Some(b"N"), false).unwrap();
        assert_eq!(s.valuedouble(item), 3.0);
        assert_eq!(get_array_size(&s, Some(obj)), 1);
    }

    #[test]
    fn detach() {
        let mut s = SafeStore::new();
        let arr = create_array(&mut s).unwrap();
        let a = create_null(&mut s).unwrap();
        let b = create_true(&mut s).unwrap();
        add_item_to_array(&mut s, arr, a);
        add_item_to_array(&mut s, arr, b);
        let d = detach_item_from_array(&mut s, arr, 0).unwrap();
        assert_eq!(d, a);
        assert_eq!(get_array_size(&s, Some(arr)), 1);
    }
}
