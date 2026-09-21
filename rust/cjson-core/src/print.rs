// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Printer matching formatted / unformatted / preallocated cJSON output.

use crate::number::format_number;
use crate::store::{NodeId, Store};
use crate::string::escape_string;
use crate::types::{self, type_kind, NESTING_LIMIT};

#[derive(Clone, Copy, Debug)]
pub struct PrintOpts {
    pub format: bool,
}

pub struct PrintError;

struct PrintBuf {
    buf: Vec<u8>,
    offset: usize,
    depth: usize,
    format: bool,
    noalloc: bool,
    length: usize,
}

impl PrintBuf {
    fn growable(format: bool) -> Self {
        Self {
            buf: vec![0; 256],
            offset: 0,
            depth: 0,
            format,
            noalloc: false,
            length: 256,
        }
    }

    fn preallocated(capacity: usize, format: bool) -> Self {
        Self {
            buf: vec![0; capacity],
            offset: 0,
            depth: 0,
            format,
            noalloc: true,
            length: capacity,
        }
    }

    fn ensure(&mut self, needed: usize) -> bool {
        if self.length > 0 && self.offset >= self.length {
            return false;
        }
        if needed > i32::MAX as usize {
            return false;
        }
        let needed_total = needed + self.offset + 1;
        if needed_total <= self.length {
            return true;
        }
        if self.noalloc {
            return false;
        }
        let newsize = if needed_total > (i32::MAX as usize) / 2 {
            if needed_total <= i32::MAX as usize {
                i32::MAX as usize
            } else {
                return false;
            }
        } else {
            needed_total * 2
        };
        self.buf.resize(newsize, 0);
        self.length = newsize;
        true
    }

    fn write(&mut self, bytes: &[u8]) -> bool {
        if !self.ensure(bytes.len()) {
            return false;
        }
        self.buf[self.offset..self.offset + bytes.len()].copy_from_slice(bytes);
        self.offset += bytes.len();
        if self.offset < self.buf.len() {
            self.buf[self.offset] = 0;
        }
        true
    }

    fn write_byte(&mut self, b: u8) -> bool {
        self.write(&[b])
    }
}

fn print_value<S: Store>(store: &S, item: NodeId, out: &mut PrintBuf) -> bool {
    match type_kind(store.ty(item)) {
        types::NULL => out.write(b"null"),
        types::FALSE => out.write(b"false"),
        types::TRUE => out.write(b"true"),
        types::NUMBER => {
            let s = format_number(store.valuedouble(item), store.valueint(item));
            out.write(s.as_bytes())
        }
        types::RAW => {
            let Some(raw) = store.valuestring_bytes(item) else {
                return false;
            };
            out.write(raw)
        }
        types::STRING => {
            let escaped = escape_string(store.valuestring_bytes(item));
            out.write(&escaped)
        }
        types::ARRAY => print_array(store, item, out),
        types::OBJECT => print_object(store, item, out),
        _ => false,
    }
}

fn print_array<S: Store>(store: &S, item: NodeId, out: &mut PrintBuf) -> bool {
    if out.depth >= NESTING_LIMIT {
        return false;
    }
    if !out.write_byte(b'[') {
        return false;
    }
    out.depth += 1;
    let mut current = store.child(item);
    while let Some(el) = current {
        if !print_value(store, el, out) {
            return false;
        }
        if store.next(el).is_some() {
            if out.format {
                if !out.write(b", ") {
                    return false;
                }
            } else if !out.write_byte(b',') {
                return false;
            }
        }
        current = store.next(el);
    }
    if !out.write_byte(b']') {
        return false;
    }
    out.depth -= 1;
    true
}

fn print_object<S: Store>(store: &S, item: NodeId, out: &mut PrintBuf) -> bool {
    if out.depth >= NESTING_LIMIT {
        return false;
    }
    if out.format {
        if !out.write(b"{\n") {
            return false;
        }
    } else if !out.write_byte(b'{') {
        return false;
    }
    out.depth += 1;
    let mut current = store.child(item);
    while let Some(el) = current {
        if out.format {
            let tabs = vec![b'\t'; out.depth];
            if !out.write(&tabs) {
                return false;
            }
        }
        let escaped = escape_string(store.key_bytes(el));
        if !out.write(&escaped) {
            return false;
        }
        if out.format {
            if !out.write(b":\t") {
                return false;
            }
        } else if !out.write_byte(b':') {
            return false;
        }
        if !print_value(store, el, out) {
            return false;
        }
        if store.next(el).is_some() {
            if !out.write_byte(b',') {
                return false;
            }
        }
        if out.format {
            if !out.write_byte(b'\n') {
                return false;
            }
        }
        current = store.next(el);
    }
    if out.format {
        let tabs = vec![b'\t'; out.depth.saturating_sub(1)];
        if !out.write(&tabs) {
            return false;
        }
    }
    if !out.write_byte(b'}') {
        return false;
    }
    out.depth -= 1;
    true
}

pub fn print<S: Store>(store: &S, item: NodeId, opts: PrintOpts) -> Option<Vec<u8>> {
    let mut buf = PrintBuf::growable(opts.format);
    if !print_value(store, item, &mut buf) {
        return None;
    }
    buf.buf.truncate(buf.offset);
    Some(buf.buf)
}

pub fn print_preallocated<S: Store>(
    store: &S,
    item: NodeId,
    dest: &mut [u8],
    format: bool,
) -> bool {
    let mut buf = PrintBuf::preallocated(dest.len(), format);
    if !print_value(store, item, &mut buf) {
        return false;
    }
    let n = buf.offset.min(dest.len());
    dest[..n].copy_from_slice(&buf.buf[..n]);
    if n < dest.len() {
        dest[n] = 0;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{parse, ParseOpts};
    use crate::store::SafeStore;

    #[test]
    fn formatted_object_uses_tabs() {
        let mut s = SafeStore::new();
        let n = parse(&mut s, b"{\"a\":1}", ParseOpts::default()).unwrap();
        let printed = print(&s, n, PrintOpts { format: true }).unwrap();
        assert_eq!(printed, b"{\n\t\"a\":\t1\n}");
    }

    #[test]
    fn unformatted_array() {
        let mut s = SafeStore::new();
        let n = parse(&mut s, b"[true,false,null]", ParseOpts::default()).unwrap();
        let printed = print(&s, n, PrintOpts { format: false }).unwrap();
        assert_eq!(printed, b"[true,false,null]");
    }
}
