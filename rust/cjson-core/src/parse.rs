// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Recursive-descent parser matching `cJSON_ParseWithLengthOpts`.

use crate::number::parse_number;
use crate::store::{NodeId, Store};
use crate::string::parse_string;
use crate::types::{self, NESTING_LIMIT};

#[derive(Clone, Copy, Debug)]
pub struct ParseOpts {
    pub require_null_terminated: bool,
    pub decimal_point: u8,
}

impl Default for ParseOpts {
    fn default() -> Self {
        Self {
            require_null_terminated: false,
            decimal_point: b'.',
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ParseError {
    pub offset: usize,
}

struct ParseBuf<'a> {
    content: &'a [u8],
    offset: usize,
    depth: usize,
}

impl<'a> ParseBuf<'a> {
    fn can_read(&self, size: usize) -> bool {
        self.offset + size <= self.content.len()
    }
    fn can_access(&self, index: usize) -> bool {
        self.offset + index < self.content.len()
    }
    fn at(&self, index: usize) -> u8 {
        self.content[self.offset + index]
    }
    fn rest(&self) -> &[u8] {
        &self.content[self.offset..]
    }
    fn skip_whitespace(&mut self) {
        if !self.can_access(0) {
            return;
        }
        while self.can_access(0) && self.at(0) <= 32 {
            self.offset += 1;
        }
        if self.offset == self.content.len() && self.offset > 0 {
            self.offset -= 1;
        }
    }
    fn skip_utf8_bom(&mut self) {
        if self.offset != 0 {
            return;
        }
        if self.can_access(4) && self.content.starts_with(b"\xEF\xBB\xBF") {
            self.offset += 3;
        }
    }
}

fn parse_array<S: Store>(
    store: &mut S,
    item: NodeId,
    buf: &mut ParseBuf<'_>,
    decimal_point: u8,
) -> bool {
    if buf.depth >= NESTING_LIMIT {
        return false;
    }
    buf.depth += 1;
    if !buf.can_access(0) || buf.at(0) != b'[' {
        return false;
    }
    buf.offset += 1;
    buf.skip_whitespace();
    if buf.can_access(0) && buf.at(0) == b']' {
        buf.depth -= 1;
        store.set_ty(item, types::ARRAY);
        store.set_child(item, None);
        buf.offset += 1;
        return true;
    }
    if !buf.can_access(0) {
        if buf.offset > 0 {
            buf.offset -= 1;
        }
        return false;
    }
    buf.offset -= 1;

    let mut head: Option<NodeId> = None;
    let mut current: Option<NodeId> = None;
    loop {
        let new_item = match store.alloc_node() {
            Some(n) => n,
            None => {
                store.delete(head);
                return false;
            }
        };
        if head.is_none() {
            head = Some(new_item);
            current = Some(new_item);
        } else if let Some(cur) = current {
            store.set_next(cur, Some(new_item));
            store.set_prev(new_item, Some(cur));
            current = Some(new_item);
        }
        buf.offset += 1;
        buf.skip_whitespace();
        if !parse_value(store, current.unwrap(), buf, decimal_point) {
            store.delete(head);
            return false;
        }
        buf.skip_whitespace();
        if !(buf.can_access(0) && buf.at(0) == b',') {
            break;
        }
    }
    if !buf.can_access(0) || buf.at(0) != b']' {
        store.delete(head);
        return false;
    }
    buf.depth -= 1;
    if let (Some(h), Some(c)) = (head, current) {
        store.set_prev(h, Some(c));
    }
    store.set_ty(item, types::ARRAY);
    store.set_child(item, head);
    buf.offset += 1;
    true
}

fn parse_object<S: Store>(
    store: &mut S,
    item: NodeId,
    buf: &mut ParseBuf<'_>,
    decimal_point: u8,
) -> bool {
    if buf.depth >= NESTING_LIMIT {
        return false;
    }
    buf.depth += 1;
    if !buf.can_access(0) || buf.at(0) != b'{' {
        return false;
    }
    buf.offset += 1;
    buf.skip_whitespace();
    if buf.can_access(0) && buf.at(0) == b'}' {
        buf.depth -= 1;
        store.set_ty(item, types::OBJECT);
        store.set_child(item, None);
        buf.offset += 1;
        return true;
    }
    if !buf.can_access(0) {
        if buf.offset > 0 {
            buf.offset -= 1;
        }
        return false;
    }
    buf.offset -= 1;

    let mut head: Option<NodeId> = None;
    let mut current: Option<NodeId> = None;
    loop {
        let new_item = match store.alloc_node() {
            Some(n) => n,
            None => {
                store.delete(head);
                return false;
            }
        };
        if head.is_none() {
            head = Some(new_item);
            current = Some(new_item);
        } else if let Some(cur) = current {
            store.set_next(cur, Some(new_item));
            store.set_prev(new_item, Some(cur));
            current = Some(new_item);
        }
        if !buf.can_access(1) {
            store.delete(head);
            return false;
        }
        buf.offset += 1;
        buf.skip_whitespace();
        let cur = current.unwrap();
        match parse_string(store, cur, buf.content, buf.offset) {
            Ok(new_off) => buf.offset = new_off,
            Err(off) => {
                buf.offset = off;
                store.delete(head);
                return false;
            }
        }
        buf.skip_whitespace();
        let key = store.valuestring_id(cur);
        store.set_key_id(cur, key);
        store.set_valuestring_id(cur, None);

        if !buf.can_access(0) || buf.at(0) != b':' {
            store.delete(head);
            return false;
        }
        buf.offset += 1;
        buf.skip_whitespace();
        if !parse_value(store, cur, buf, decimal_point) {
            store.delete(head);
            return false;
        }
        buf.skip_whitespace();
        if !(buf.can_access(0) && buf.at(0) == b',') {
            break;
        }
    }
    if !buf.can_access(0) || buf.at(0) != b'}' {
        store.delete(head);
        return false;
    }
    buf.depth -= 1;
    if let (Some(h), Some(c)) = (head, current) {
        store.set_prev(h, Some(c));
    }
    store.set_ty(item, types::OBJECT);
    store.set_child(item, head);
    buf.offset += 1;
    true
}

fn parse_value<S: Store>(
    store: &mut S,
    item: NodeId,
    buf: &mut ParseBuf<'_>,
    decimal_point: u8,
) -> bool {
    if buf.content.is_empty() && buf.offset == 0 {
        return false;
    }
    if buf.can_read(4) && buf.rest().starts_with(b"null") {
        store.set_ty(item, types::NULL);
        buf.offset += 4;
        return true;
    }
    if buf.can_read(5) && buf.rest().starts_with(b"false") {
        store.set_ty(item, types::FALSE);
        buf.offset += 5;
        return true;
    }
    if buf.can_read(4) && buf.rest().starts_with(b"true") {
        store.set_ty(item, types::TRUE);
        store.set_valueint(item, 1);
        buf.offset += 4;
        return true;
    }
    if buf.can_access(0) && buf.at(0) == b'"' {
        return match parse_string(store, item, buf.content, buf.offset) {
            Ok(off) => {
                buf.offset = off;
                true
            }
            Err(off) => {
                buf.offset = off;
                false
            }
        };
    }
    if buf.can_access(0) && (buf.at(0) == b'-' || buf.at(0).is_ascii_digit()) {
        return match parse_number(store, item, buf.content, buf.offset, decimal_point) {
            Some(off) => {
                buf.offset = off;
                true
            }
            None => false,
        };
    }
    if buf.can_access(0) && buf.at(0) == b'[' {
        return parse_array(store, item, buf, decimal_point);
    }
    if buf.can_access(0) && buf.at(0) == b'{' {
        return parse_object(store, item, buf, decimal_point);
    }
    false
}

pub fn parse<S: Store>(
    store: &mut S,
    input: &[u8],
    opts: ParseOpts,
) -> Result<NodeId, ParseError> {
    if input.is_empty() {
        return Err(ParseError { offset: 0 });
    }
    let mut buf = ParseBuf {
        content: input,
        offset: 0,
        depth: 0,
    };
    let item = store.alloc_node().ok_or(ParseError { offset: 0 })?;
    buf.skip_utf8_bom();
    buf.skip_whitespace();
    if !parse_value(store, item, &mut buf, opts.decimal_point) {
        store.delete(Some(item));
        let pos = if buf.offset < input.len() {
            buf.offset
        } else if !input.is_empty() {
            input.len() - 1
        } else {
            0
        };
        return Err(ParseError { offset: pos });
    }
    if opts.require_null_terminated {
        buf.skip_whitespace();
        if buf.offset >= input.len() || buf.at(0) != 0 {
            store.delete(Some(item));
            let pos = if buf.offset < input.len() {
                buf.offset
            } else if !input.is_empty() {
                input.len() - 1
            } else {
                0
            };
            return Err(ParseError { offset: pos });
        }
    }
    Ok(item)
}

/// Byte offset of the first byte after the last successfully parsed value.
pub fn parse_with_end<S: Store>(
    store: &mut S,
    input: &[u8],
    opts: ParseOpts,
) -> Result<(NodeId, usize), ParseError> {
    if input.is_empty() {
        return Err(ParseError { offset: 0 });
    }
    let mut buf = ParseBuf {
        content: input,
        offset: 0,
        depth: 0,
    };
    let item = store.alloc_node().ok_or(ParseError { offset: 0 })?;
    buf.skip_utf8_bom();
    buf.skip_whitespace();
    if !parse_value(store, item, &mut buf, opts.decimal_point) {
        store.delete(Some(item));
        let pos = if buf.offset < input.len() {
            buf.offset
        } else if !input.is_empty() {
            input.len() - 1
        } else {
            0
        };
        return Err(ParseError { offset: pos });
    }
    if opts.require_null_terminated {
        buf.skip_whitespace();
        if buf.offset >= input.len() || buf.at(0) != 0 {
            store.delete(Some(item));
            let pos = if buf.offset < input.len() {
                buf.offset
            } else if !input.is_empty() {
                input.len() - 1
            } else {
                0
            };
            return Err(ParseError { offset: pos });
        }
    }
    Ok((item, buf.offset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::print::{print, PrintOpts};
    use crate::store::SafeStore;

    #[test]
    fn parse_literals() {
        let mut s = SafeStore::new();
        let n = parse(&mut s, b"null", ParseOpts::default()).unwrap();
        assert_eq!(s.ty(n), types::NULL);
        let n = parse(&mut s, b"true", ParseOpts::default()).unwrap();
        assert_eq!(s.ty(n), types::TRUE);
        assert_eq!(s.valueint(n), 1);
        let n = parse(&mut s, b"false", ParseOpts::default()).unwrap();
        assert_eq!(s.ty(n), types::FALSE);
    }

    #[test]
    fn parse_array_object() {
        let mut s = SafeStore::new();
        let n = parse(&mut s, b"[1,2]", ParseOpts::default()).unwrap();
        assert_eq!(s.ty(n), types::ARRAY);
        let printed = print(&s, n, PrintOpts { format: false }).unwrap();
        assert_eq!(printed, b"[1,2]");

        let n = parse(&mut s, b"{\"a\":true}", ParseOpts::default()).unwrap();
        let printed = print(&s, n, PrintOpts { format: false }).unwrap();
        assert_eq!(printed, b"{\"a\":true}");
    }

    #[test]
    fn empty_fails() {
        let mut s = SafeStore::new();
        assert!(parse(&mut s, b"", ParseOpts::default()).is_err());
    }

    #[test]
    fn nesting_limit() {
        let mut deep = String::new();
        for _ in 0..1001 {
            deep.push('[');
        }
        for _ in 0..1001 {
            deep.push(']');
        }
        let mut s = SafeStore::new();
        assert!(parse(&mut s, deep.as_bytes(), ParseOpts::default()).is_err());
    }

    #[test]
    fn c_golden_examples() {
        let inputs = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/inputs");
        if !inputs.exists() {
            return;
        }
        for name in ["test1", "test2", "test3", "test4", "test5", "test7", "test8", "test9", "test10", "test11"] {
            let src = std::fs::read(inputs.join(name)).expect(name);
            let expected = std::fs::read(inputs.join(format!("{name}.expected"))).expect(name);
            let mut s = SafeStore::new();
            let n = parse(&mut s, &src, ParseOpts::default()).unwrap_or_else(|_| panic!("parse {name}"));
            let actual = print(&s, n, PrintOpts { format: true }).unwrap();
            let expected = String::from_utf8_lossy(&expected);
            let actual = String::from_utf8_lossy(&actual);
            assert_eq!(actual.trim_end(), expected.trim_end(), "golden {name}");
        }
    }
}
