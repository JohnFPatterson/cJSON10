// Copyright (c) 2009-2017 Dave Gamble and cJSON contributors
// Copyright (c) 2026 cJSON Rust port contributors

//! In-place minify matching `cJSON_Minify` (strips comments and whitespace).

/// Minify a NUL-terminated buffer in place. Returns the new content length (not including NUL).
pub fn minify(buf: &mut [u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }
    let mut input = 0usize;
    let mut output = 0usize;
    while input < buf.len() && buf[input] != 0 {
        match buf[input] {
            b' ' | b'\t' | b'\r' | b'\n' => input += 1,
            b'/' => {
                if input + 1 < buf.len() && buf[input + 1] == b'/' {
                    input += 2;
                    while input < buf.len() && buf[input] != 0 {
                        if buf[input] == b'\n' {
                            input += 1;
                            break;
                        }
                        input += 1;
                    }
                } else if input + 1 < buf.len() && buf[input + 1] == b'*' {
                    input += 2;
                    while input < buf.len() && buf[input] != 0 {
                        if input + 1 < buf.len() && buf[input] == b'*' && buf[input + 1] == b'/' {
                            input += 2;
                            break;
                        }
                        input += 1;
                    }
                } else {
                    input += 1;
                }
            }
            b'"' => {
                buf[output] = buf[input];
                input += 1;
                output += 1;
                while input < buf.len() && buf[input] != 0 {
                    buf[output] = buf[input];
                    if buf[input] == b'"' {
                        input += 1;
                        output += 1;
                        break;
                    } else if input + 1 < buf.len() && buf[input] == b'\\' && buf[input + 1] == b'"'
                    {
                        buf[output + 1] = buf[input + 1];
                        input += 1;
                        output += 1;
                    }
                    if input < buf.len() && buf[input] != 0 {
                        input += 1;
                        output += 1;
                    }
                }
            }
            c => {
                buf[output] = c;
                input += 1;
                output += 1;
            }
        }
    }
    if output < buf.len() {
        buf[output] = 0;
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minify_str(s: &str) -> String {
        let mut buf = s.as_bytes().to_vec();
        buf.push(0);
        let n = minify(&mut buf);
        String::from_utf8(buf[..n].to_vec()).unwrap()
    }

    #[test]
    fn comments_and_spaces() {
        assert_eq!(minify_str("{// comment\n}"), "{}");
        assert_eq!(minify_str("{ \"key\":\ttrue\r\n    }"), "{\"key\":true}");
        assert_eq!(
            minify_str("{/* this is\n a /* multi\n //line \n {comment \"\\\" */}"),
            "{}"
        );
        assert_eq!(minify_str("/* bla"), "");
        assert_eq!(minify_str("\"\\"), "\"\\");
    }
}
