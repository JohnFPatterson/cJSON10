// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! String parse (including `\uXXXX` surrogates) and JSON string print.

use crate::store::{NodeId, Store};
use crate::types;

pub fn parse_hex4(input: &[u8]) -> Option<u32> {
    if input.len() < 4 {
        return None;
    }
    let mut h = 0u32;
    for i in 0..4 {
        let c = input[i];
        if (b'0'..=b'9').contains(&c) {
            h += u32::from(c - b'0');
        } else if (b'A'..=b'F').contains(&c) {
            h += 10 + u32::from(c - b'A');
        } else if (b'a'..=b'f').contains(&c) {
            h += 10 + u32::from(c - b'a');
        } else {
            return Some(0);
        }
        if i < 3 {
            h <<= 4;
        }
    }
    Some(h)
}

/// Convert a `\uXXXX` or surrogate pair starting at `input[0] == b'\\'`.
/// Returns (utf8 bytes, input sequence length).
pub fn utf16_literal_to_utf8(input: &[u8]) -> Option<(Vec<u8>, usize)> {
    if input.len() < 6 {
        return None;
    }
    let first_code = parse_hex4(&input[2..6])?;
    if (0xDC00..=0xDFFF).contains(&first_code) {
        return None;
    }
    let (codepoint, sequence_length) = if (0xD800..=0xDBFF).contains(&first_code) {
        if input.len() < 12 {
            return None;
        }
        if input[6] != b'\\' || input[7] != b'u' {
            return None;
        }
        let second_code = parse_hex4(&input[8..12])?;
        if !(0xDC00..=0xDFFF).contains(&second_code) {
            return None;
        }
        let cp = 0x10000 + (((first_code & 0x3FF) << 10) | (second_code & 0x3FF));
        (cp, 12usize)
    } else {
        (first_code, 6usize)
    };

    let mut out = Vec::new();
    match char::from_u32(codepoint) {
        Some(c) => {
            let mut buf = [0u8; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
        None => return None,
    }
    Some((out, sequence_length))
}

/// Parse a JSON string at `offset` (must start with `"`). On success, sets item to String.
pub fn parse_string<S: Store>(
    store: &mut S,
    item: NodeId,
    input: &[u8],
    offset: usize,
) -> Result<usize, usize> {
    if offset >= input.len() || input[offset] != b'"' {
        return Err(offset);
    }
    let mut input_end = offset + 1;
    let mut skipped = 0usize;
    while input_end < input.len() && input[input_end] != b'"' {
        if input[input_end] == b'\\' {
            if input_end + 1 >= input.len() {
                return Err(offset + 1);
            }
            skipped += 1;
            input_end += 1;
        }
        input_end += 1;
    }
    if input_end >= input.len() || input[input_end] != b'"' {
        return Err(offset + 1);
    }

    let mut output = Vec::with_capacity(input_end - offset - skipped);
    let mut p = offset + 1;
    while p < input_end {
        if input[p] != b'\\' {
            output.push(input[p]);
            p += 1;
            continue;
        }
        if p + 1 >= input_end {
            return Err(p);
        }
        match input[p + 1] {
            b'b' => output.push(b'\x08'),
            b'f' => output.push(b'\x0c'),
            b'n' => output.push(b'\n'),
            b'r' => output.push(b'\r'),
            b't' => output.push(b'\t'),
            b'"' | b'\\' | b'/' => output.push(input[p + 1]),
            b'u' => match utf16_literal_to_utf8(&input[p..input_end]) {
                Some((bytes, seq)) => {
                    output.extend_from_slice(&bytes);
                    p += seq;
                    continue;
                }
                None => return Err(p),
            },
            _ => return Err(p),
        }
        p += 2;
    }

    let sid = store.alloc_str(&output).ok_or(offset)?;
    store.set_ty(item, types::STRING);
    store.set_valuestring_id(item, Some(sid));
    Ok(input_end + 1)
}

pub fn escape_string(input: Option<&[u8]>) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(b'"');
    let Some(input) = input else {
        out.push(b'"');
        return out;
    };
    for &c in input {
        match c {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\x08' => out.extend_from_slice(b"\\b"),
            b'\x0c' => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            c if c < 32 => {
                out.extend_from_slice(format!("\\u{c:04x}").as_bytes());
            }
            c => out.push(c),
        }
    }
    out.push(b'"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{SafeStore, Store};

    fn parse_ok(literal: &str) -> Vec<u8> {
        let mut s = SafeStore::new();
        let n = s.alloc_node().unwrap();
        let input = literal.as_bytes();
        let end = parse_string(&mut s, n, input, 0).unwrap();
        assert_eq!(end, input.len());
        s.valuestring_bytes(n).unwrap().to_vec()
    }

    #[test]
    fn hex4() {
        assert_eq!(parse_hex4(b"0000"), Some(0));
        assert_eq!(parse_hex4(b"FFFF"), Some(0xFFFF));
        assert_eq!(parse_hex4(b"abcd"), Some(0xABCD));
    }

    #[test]
    fn parse_fixtures() {
        assert_eq!(parse_ok("\"\""), b"");
        assert_eq!(
            parse_ok("\"\\\"\\\\\\/\\b\\f\\n\\r\\t\\u20AC\\u732b\""),
            "\"\\/\u{0008}\u{000c}\n\r\t€猫".as_bytes()
        );
        assert_eq!(parse_ok("\"\\uD83D\\udc31\""), "🐱".as_bytes());
        assert_eq!(parse_ok("\"\u{0008}\u{000c}\n\r\t\""), b"\x08\x0c\n\r\t");
    }

    #[test]
    fn reject_bad() {
        let mut s = SafeStore::new();
        let n = s.alloc_node().unwrap();
        assert!(parse_string(&mut s, n, b"this\" is not a string\"", 0).is_err());
        assert!(parse_string(&mut s, n, b"", 0).is_err());
        assert!(parse_string(&mut s, n, b"\"000000000000000000\\", 0).is_err());
    }

    #[test]
    fn bug_94() {
        let got = parse_ok("\"~!@\\\\#$%^&*()\\\\\\\\-\\\\+{}[]:\\\\;\\\\\\\"\\\\<\\\\>?/.,DC=ad,DC=com\"");
        assert_eq!(
            got,
            b"~!@\\#$%^&*()\\\\-\\+{}[]:\\;\\\"\\<\\>?/.,DC=ad,DC=com"
        );
    }
}
