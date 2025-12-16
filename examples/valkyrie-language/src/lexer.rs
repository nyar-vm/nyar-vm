#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Ident(String),
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Plus,
    Minus,
    Star,
    Slash,
    DoubleEq, // ==
    NotEq,    // !=
    Lt,       // <
    Le,       // <=
    Gt,       // >
    Ge,       // >=
    And,      // &&
    Or,       // ||
    Not,      // !
    Eq,
    Pipe,
    Dot,
    Colon,
    DoubleColon,
    Namespace,
    Using,
    Let,
    Micro,
    Return,
    Yield,
    If,
    Else,
    While,
    Loop,
    Break,
    Continue,
    Class,
    New,
    Is,
    As,
    AsSafe,
    Typeof,
    Trait,
    Imply,
    Impl,
    For,
    // ADT & Pattern Matching
    Enum,
    Match,
    Case,
    Arrow,      // =>
    Underscore, // _
    Assert,
    Debug,
    True,
    False,
    Int(i64),
    String(String),
    Eof,
}

#[derive(Debug)]
pub enum Error {
    Lex(String),
    Parse(String),
    Compile(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Lex(s) => write!(f, "lex error: {}", s),
            Error::Parse(s) => write!(f, "parse error: {}", s),
            Error::Compile(s) => write!(f, "compile error: {}", s),
        }
    }
}

impl std::error::Error for Error {}

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
            match s {
                "namespace" => out.push((Token::Namespace, line)),
                "using" => out.push((Token::Using, line)),
                "let" => out.push((Token::Let, line)),
                "micro" => out.push((Token::Micro, line)),
                "return" => out.push((Token::Return, line)),
                "yield" => out.push((Token::Yield, line)),
                "if" => out.push((Token::If, line)),
                "else" => out.push((Token::Else, line)),
                "while" => out.push((Token::While, line)),
                "loop" => out.push((Token::Loop, line)),
                "break" => out.push((Token::Break, line)),
                "continue" => out.push((Token::Continue, line)),
                "class" => out.push((Token::Class, line)),
                "new" => out.push((Token::New, line)),
                "is" => out.push((Token::Is, line)),
                "as" => {
                    if i < b.len() && b[i] == b'?' {
                        i += 1;
                        out.push((Token::AsSafe, line));
                    } else {
                        out.push((Token::As, line));
                    }
                }
                "typeof" => out.push((Token::Typeof, line)),
                "trait" => out.push((Token::Trait, line)),
                "imply" => out.push((Token::Imply, line)),
                "impl" => out.push((Token::Impl, line)),
                "for" => out.push((Token::For, line)),
                "enum" => out.push((Token::Enum, line)),
                "match" => out.push((Token::Match, line)),
                "case" => out.push((Token::Case, line)),
                "assert" => out.push((Token::Assert, line)),
                "debug" => out.push((Token::Debug, line)),
                "true" => out.push((Token::True, line)),
                "false" => out.push((Token::False, line)),
                _ => out.push((Token::Ident(s.to_string()), line)),
            }
            continue;
        }
        return Err(Error::Lex(format!("unexpected byte {}", c)));
    }
    out.push((Token::Eof, line));
    Ok(out)
}
