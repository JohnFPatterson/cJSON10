// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Type flags matching `cJSON.h`.

pub const VERSION_MAJOR: u32 = 1;
pub const VERSION_MINOR: u32 = 7;
pub const VERSION_PATCH: u32 = 19;
pub const VERSION_STRING: &str = "1.7.19";

pub const INVALID: i32 = 0;
pub const FALSE: i32 = 1 << 0;
pub const TRUE: i32 = 1 << 1;
pub const NULL: i32 = 1 << 2;
pub const NUMBER: i32 = 1 << 3;
pub const STRING: i32 = 1 << 4;
pub const ARRAY: i32 = 1 << 5;
pub const OBJECT: i32 = 1 << 6;
pub const RAW: i32 = 1 << 7;

pub const IS_REFERENCE: i32 = 256;
pub const STRING_IS_CONST: i32 = 512;

pub const NESTING_LIMIT: usize = 1000;
pub const CIRCULAR_LIMIT: usize = 10000;

pub const TYPE_MASK: i32 = 0xFF;

#[inline]
pub fn type_kind(ty: i32) -> i32 {
    ty & TYPE_MASK
}

#[inline]
pub fn is_reference(ty: i32) -> bool {
    ty & IS_REFERENCE != 0
}

#[inline]
pub fn string_is_const(ty: i32) -> bool {
    ty & STRING_IS_CONST != 0
}

/// POSIX C-locale `tolower` (ASCII only), matching the default cJSON test locale.
#[inline]
pub fn c_tolower(b: u8) -> u8 {
    if (b'A'..=b'Z').contains(&b) {
        b + 32
    } else {
        b
    }
}

/// Case-insensitive byte compare. Two NULL inputs are not equal (returns 1), matching cJSON.
pub fn case_insensitive_strcmp(a: Option<&[u8]>, b: Option<&[u8]>) -> i32 {
    match (a, b) {
        (None, _) | (_, None) => 1,
        (Some(a), Some(b)) => {
            if std::ptr::eq(a, b) {
                return 0;
            }
            let n = a.len().min(b.len());
            for i in 0..n {
                let ca = c_tolower(a[i]);
                let cb = c_tolower(b[i]);
                if ca != cb {
                    return i32::from(ca) - i32::from(cb);
                }
            }
            if a.len() == b.len() {
                0
            } else if a.len() < b.len() {
                i32::from(0) - i32::from(c_tolower(b[n]))
            } else {
                i32::from(c_tolower(a[n]))
            }
        }
    }
}

pub fn bytes_eq_case(a: Option<&[u8]>, b: Option<&[u8]>, case_sensitive: bool) -> bool {
    if case_sensitive {
        match (a, b) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    } else {
        case_insensitive_strcmp(a, b) == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolower_ascii() {
        assert_eq!(c_tolower(b'A'), b'a');
        assert_eq!(c_tolower(b'z'), b'z');
        assert_eq!(c_tolower(0xC0), 0xC0);
    }

    #[test]
    fn case_insensitive_nulls_are_unequal() {
        assert_eq!(case_insensitive_strcmp(None, None), 1);
        assert_eq!(case_insensitive_strcmp(Some(b"a"), None), 1);
    }

    #[test]
    fn case_insensitive_match() {
        assert_eq!(case_insensitive_strcmp(Some(b"Foo"), Some(b"foo")), 0);
        assert_ne!(case_insensitive_strcmp(Some(b"Foo"), Some(b"bar")), 0);
    }
}
