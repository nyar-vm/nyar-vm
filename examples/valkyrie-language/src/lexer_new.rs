
use crate::lexer::{Error, Token};

pub fn lex(input: &str) -> Result<Vec<(Token, u32)>, Error> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1u32;
    let b = input.as_bytes();
    while i < b.len() {
        let c = b[i];
        if c == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c == b' ' || c == b'\r' || c == b'\t' {
            i += 1;
            continue;
        }
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c == b'/' {
            out.push((Token::Slash, line));
            i += 1;
            continue;
        }
        if c == b'*' {
            out.push((Token::Star, line));
            i += 1;
            continue;
        }
        if c == b':' {
            if i + 1 < b.len() && b[i + 1] == b':' {
                out.push((Token::DoubleColon, line));
                i += 2;
                continue;
            } else {
                out.push((Token::Colon, line));
                i += 1;
                continue;
            }
        }
        if c == b'(' {
            out.push((Token::LParen, line));
            i += 1;
            continue;
        }
        if c == b')' {
            out.push((Token::RParen, line));
            i += 1;
            continue;
        }
        if c == b'{' {
            out.push((Token::LBrace, line));
            i += 1;
            continue;
        }
        if c == b'}' {
            out.push((Token::RBrace, line));
            i += 1;
            continue;
        }
        if c == b',' {
            out.push((Token::Comma, line));
            i += 1;
            continue;
        }
        if c == b';' {
            out.push((Token::Semi, line));
            i += 1;
            continue;
        }
        if c == b'+' {
            out.push((Token::Plus, line));
            i += 1;
            continue;
        }
        if c == b'-' {
            if i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit() {
                let start = i;
                i += 1;
                while i < b.len() && (b[i] as char).is_ascii_digit() {
                    i += 1;
                }
                let s = std::str::from_utf8(&b[start..i])
                    .map_err(|_| Error::Lex("utf8".to_string()))?;
                let v = s
                    .parse::<i64>()
                    .map_err(|_| Error::Lex("int".to_string()))?;
                out.push((Token::Int(v), line));
                continue;
            }
            out.push((Token::Minus, line));
            i += 1;
            continue;
        }
        if c == b'!' {
            if i + 1 < b.len() && b[i + 1] == b'=' {
                out.push((Token::NotEq, line));
                i += 2;
                continue;
            }
            out.push((Token::Not, line));
            i += 1;
            continue;
        }
        if c == b'<' {
            if i + 1 < b.len() && b[i + 1] == b'=' {
                out.push((Token::Le, line));
                i += 2;
                continue;
            }
            out.push((Token::Lt, line));
            i += 1;
            continue;
        }
        if c == b'>' {
            if i + 1 < b.len() && b[i + 1] == b'=' {
                out.push((Token::Ge, line));
                i += 2;
                continue;
            }
            out.push((Token::Gt, line));
            i += 1;
            continue;
        }
        if c == b'&' {
            if i + 1 < b.len() && b[i + 1] == b'&' {
                out.push((Token::And, line));
                i += 2;
                continue;
            }
            return Err(Error::Lex(format!("unexpected byte {}", c)));
        }
        if c == b'=' {
            if i + 1 < b.len() && b[i + 1] == b'>' {
                out.push((Token::Arrow, line));
                i += 2;
                continue;
            }
            if i + 1 < b.len() && b[i + 1] == b'=' {
                out.push((Token::DoubleEq, line));
                i += 2;
                continue;
            }
            out.push((Token::Eq, line));
            i += 1;
            continue;
        }
        if c == b'|' {
            if i + 1 < b.len() && b[i + 1] == b'|' {
                out.push((Token::Or, line));
                i += 2;
                continue;
            }
            out.push((Token::Pipe, line));
            i += 1;
            continue;
        }
        if c == b'.' {
            out.push((Token::Dot, line));
            i += 1;
            continue;
        }
        if c == b'_' {
            // Check if it's a standalone underscore or start of identifier
            if i + 1 < b.len() && ((b[i + 1] as char).is_ascii_alphanumeric() || b[i + 1] == b'_') {
                // It's an identifier starting with _, fall through to identifier parsing
            } else {
                out.push((Token::Underscore, line));
                i += 1;
                continue;
            }
        }

        if c == b'"' {
            i += 1;
            let start = i;
            while i < b.len() && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < b.len() {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if i >= b.len() {
                return Err(Error::Lex("unterminated string".to_string()));
            }
            let s_raw =
                std::str::from_utf8(&b[start..i]).map_err(|_| Error::Lex("utf8".to_string()))?;
            // Simple unescape
            let s = s_raw
                .replace("\\\"", "\"")
                .replace("\\n", "\n")
                .replace("\\t", "\t")
                .replace("\\r", "\r")
                .replace("\\0", "\0")
                .replace("\\\\", "\\");
            out.push((Token::String(s), line));
            i += 1;
            continue;
        }
        if (c as char).is_ascii_digit() {
            if c == b'0' && i + 1 < b.len() && (b[i + 1] == b'x' || b[i + 1] == b'X') {
                i += 2;
                let start = i;
                while i < b.len() && (b[i] as char).is_ascii_hexdigit() {
                    i += 1;
                }
                if start == i {
                    return Err(Error::Lex("expected hex digits after 0x".to_string()));
                }
                let s = std::str::from_utf8(&b[start..i])
                    .map_err(|_| Error::Lex("utf8".to_string()))?;
                let v = i64::from_str_radix(s, 16).map_err(|_| Error::Lex("int".to_string()))?;
                out.push((Token::Int(v), line));
                continue;
            } else {
                let start = i;
                i += 1;
                while i < b.len() && (b[i] as char).is_ascii_digit() {
                    i += 1;
                }
                let s = std::str::from_utf8(&b[start..i])
                    .map_err(|_| Error::Lex("utf8".to_string()))?;
                let v = s
                    .parse::<i64>()
                    .map_err(|_| Error::Lex("int".to_string()))?;
                out.push((Token::Int(v), line));
                continue;
            }
        }
        if (c as char).is_ascii_alphabetic() || c == b'_' {
            let start = i;
            i += 1;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let s =
                std::str::from_utf8(&b[start..i]).map_err(|_| Error::Lex("utf8".to_string()))?;
            let t = match s {
                "namespace" => Token::Namespace,
                "using" => Token::Using,
                "let" => Token::Let,
                "micro" => Token::Micro,
                "return" => Token::Return,
                "yield" => Token::Yield,
                "if" => Token::If,
                "else" => Token::Else,
                "while" => Token::While,
                "loop" => Token::Loop,
                "break" => Token::Break,
                "continue" => Token::Continue,
                "class" => Token::Class,
                "new" => Token::New,
                "is" => Token::Is,
                "as" => {
                    if i < b.len() && b[i] == b'?' {
                        i += 1;
                        Token::AsSafe
                    } else {
                        Token::As
                    }
                }
                "typeof" => Token::Typeof,
                "trait" => Token::Trait,
                "imply" => Token::Imply,
                "impl" => Token::Impl,
                "for" => Token::For,
                "enum" => Token::Enum,
                "match" => Token::Match,
                "case" => Token::Case,
                "assert" => Token::Assert,
                "debug" => Token::Debug,
                "true" => Token::True,
                "false" => Token::False,
                _ => Token::Ident(s.to_string()),
            };
            out.push((t, line));
            continue;
        }
        return Err(Error::Lex(format!("unexpected byte {}", c)));
    }
    out.push((Token::Eof, line));
    Ok(out)
}
