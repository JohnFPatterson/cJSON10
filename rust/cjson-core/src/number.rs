// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! Number parse/print matching cJSON's `strtod` / `%1.15g` / `%1.17g` path.

use crate::store::{NodeId, Store};
use crate::types;

const DBL_EPSILON: f64 = f64::EPSILON;

#[inline]
pub fn compare_double(a: f64, b: f64) -> bool {
    let max_val = a.abs().max(b.abs());
    (a - b).abs() <= max_val * DBL_EPSILON
}

#[inline]
pub fn saturate_int(number: f64) -> i32 {
    if number >= i32::MAX as f64 {
        i32::MAX
    } else if number <= i32::MIN as f64 {
        i32::MIN
    } else {
        number as i32
    }
}

/// Scan a C-style number token (digits, sign, exponent, decimal point).
pub fn scan_number_token(input: &[u8], start: usize) -> (usize, bool) {
    let mut i = start;
    let mut has_decimal = false;
    while i < input.len() {
        match input[i] {
            b'0'..=b'9' | b'+' | b'-' | b'e' | b'E' => i += 1,
            b'.' => {
                has_decimal = true;
                i += 1;
            }
            _ => break,
        }
    }
    (i, has_decimal)
}

/// Prefix of `s` that `strtod` would accept (sign, digits, fraction, exponent).
fn strtod_prefix(s: &[u8]) -> Option<&[u8]> {
    let mut i = 0;
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        i += 1;
    }
    let start_digits = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let mut saw_digit = i > start_digits;
    if i < s.len() && s[i] == b'.' {
        i += 1;
        let frac = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        saw_digit |= i > frac;
    }
    if !saw_digit {
        return None;
    }
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let e = i;
        i += 1;
        if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
            i += 1;
        }
        let exp = i;
        while i < s.len() && s[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp {
            i = e;
        }
    }
    Some(&s[..i])
}

pub fn parse_strtod(token: &[u8]) -> Option<(f64, usize)> {
    let prefix = strtod_prefix(token)?;
    if prefix.is_empty() {
        return None;
    }
    let text = core::str::from_utf8(prefix).ok()?;
    let value: f64 = text.parse().ok()?;
    Some((value, prefix.len()))
}

/// Parse a number at `offset` into `item`. Returns the new offset on success.
pub fn parse_number<S: Store>(
    store: &mut S,
    item: NodeId,
    input: &[u8],
    offset: usize,
    _decimal_point: u8,
) -> Option<usize> {
    if offset >= input.len() {
        return None;
    }
    let (end, _has_dot) = scan_number_token(input, offset);
    if end == offset {
        return None;
    }
    let token = &input[offset..end];
    let (number, consumed) = parse_strtod(token)?;
    store.set_valuedouble(item, number);
    store.set_valueint(item, saturate_int(number));
    store.set_ty(item, types::NUMBER);
    Some(offset + consumed)
}

/// C `%g`-style formatting with `prec` significant digits and a 2-digit exponent.
pub fn format_g(value: f64, prec: usize) -> String {
    if value == 0.0 {
        // Preserve sign of zero for the token scan; cJSON prints 0 / -0 via %d when valueint matches.
        return if value.is_sign_negative() {
            "-0".into()
        } else {
            "0".into()
        };
    }
    let prec = prec.max(1);
    let abs = value.abs();
    let exp = abs.log10().floor() as i32;
    if exp < -4 || exp >= prec as i32 {
        format_e(value, prec)
    } else {
        format_f_from_sig(value, prec, exp)
    }
}

fn format_e(value: f64, prec: usize) -> String {
    let digits_after = prec.saturating_sub(1);
    let s = format!("{:.*e}", digits_after, value);
    normalize_e(&s)
}

fn normalize_e(s: &str) -> String {
    // Split at 'e'
    let (mant, exp) = match s.split_once('e') {
        Some(p) => p,
        None => return s.to_string(),
    };
    let mant = strip_trailing_zeros_keep_dotless(mant);
    let exp_i: i32 = exp.parse().unwrap_or(0);
    format!("{}e{:+03}", mant, exp_i)
}

fn strip_trailing_zeros_keep_dotless(mant: &str) -> String {
    if !mant.contains('.') {
        return mant.to_string();
    }
    let mut out = mant.to_string();
    while out.ends_with('0') {
        out.pop();
    }
    if out.ends_with('.') {
        out.pop();
    }
    out
}

fn format_f_from_sig(value: f64, prec: usize, exp: i32) -> String {
    // Number of digits after the decimal so that total significant digits == prec.
    // For 123.45, exp=2, prec=15 → 12 fraction digits, then strip trailing zeros.
    let frac = if exp >= 0 {
        prec.saturating_sub((exp as usize) + 1)
    } else {
        prec + ((-exp) as usize) - 1
    };
    let s = format!("{:.*}", frac, value);
    strip_trailing_zeros_keep_dotless(&s)
}

pub fn format_number(valuedouble: f64, valueint: i32) -> String {
    if !valuedouble.is_finite() {
        return "null".into();
    }
    if valuedouble == valueint as f64 {
        return format!("{valueint}");
    }
    let first = format_g(valuedouble, 15);
    if let Ok(test) = first.parse::<f64>() {
        if compare_double(test, valuedouble) {
            return first;
        }
    }
    format_g(valuedouble, 17)
}

pub fn set_number_fields<S: Store>(store: &mut S, item: NodeId, number: f64) {
    store.set_valueint(item, saturate_int(number));
    store.set_valuedouble(item, number);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SafeStore;

    #[test]
    fn parse_basic_ints() {
        let mut s = SafeStore::new();
        let n = s.alloc_node().unwrap();
        let off = parse_number(&mut s, n, b"2147483647", 0, b'.').unwrap();
        assert_eq!(off, 10);
        assert_eq!(s.valueint(n), i32::MAX);
        assert_eq!(s.valuedouble(n), 2147483647.0);
    }

    #[test]
    fn parse_reals_and_saturation() {
        let mut s = SafeStore::new();
        let n = s.alloc_node().unwrap();
        parse_number(&mut s, n, b"10e10", 0, b'.').unwrap();
        assert_eq!(s.valueint(n), i32::MAX);
        assert_eq!(s.valuedouble(n), 10e10);

        parse_number(&mut s, n, b"-10e20", 0, b'.').unwrap();
        assert_eq!(s.valueint(n), i32::MIN);
        assert_eq!(s.valuedouble(n), -10e20);

        parse_number(&mut s, n, b"-0", 0, b'.').unwrap();
        assert_eq!(s.valueint(n), 0);
        assert_eq!(s.valuedouble(n), -0.0);

        parse_number(&mut s, n, b"0.001", 0, b'.').unwrap();
        assert_eq!(s.valueint(n), 0);
        assert_eq!(s.valuedouble(n), 0.001);
    }

    #[test]
    fn print_matches_cjson_fixtures() {
        assert_eq!(format_number(0.0, 0), "0");
        assert_eq!(format_number(-1.0, -1), "-1");
        assert_eq!(format_number(-32768.0, -32768), "-32768");
        assert_eq!(format_number(-2147483648.0, i32::MIN), "-2147483648");
        assert_eq!(format_number(1.0, 1), "1");
        assert_eq!(format_number(32767.0, 32767), "32767");
        assert_eq!(format_number(2147483647.0, i32::MAX), "2147483647");
        assert_eq!(format_number(0.123, 0), "0.123");
        assert_eq!(format_number(10e-10, 0), "1e-09");
        assert_eq!(format_number(10e11, i32::MAX), "1000000000000");
        assert_eq!(format_number(123e+127, i32::MAX), "1.23e+129");
        assert_eq!(format_number(123e-128, 0), "1.23e-126");
        assert_eq!(format_number(-0.0123, 0), "-0.0123");
        assert_eq!(format_number(-10e-10, 0), "-1e-09");
        assert_eq!(format_number(-10e20, i32::MIN), "-1e+21");
        assert_eq!(format_number(f64::NAN, 0), "null");
        assert_eq!(format_number(f64::INFINITY, 0), "null");
    }

    #[test]
    fn print_pi() {
        let printed = format_number(3.1415926535897931, 3);
        assert_eq!(printed, "3.1415926535897931");
    }
}
