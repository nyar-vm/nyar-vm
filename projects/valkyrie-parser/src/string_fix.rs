use std::iter::Iterator;
use std::{char, str};

pub fn unescape(s: &str) -> String {
    let mut chars = s.chars().enumerate();
    let mut res = String::with_capacity(s.len());
    while let Some((_, c)) = chars.next() {
        // when in a single quote, no escapes are possible
        if c == '\\' {
            match chars.next() {
                None => ' ',
                Some((_, c2)) => {
                    res.push(match c2 {
                        'a' => '\u{07}',
                        'b' => '\u{08}',
                        'v' => '\u{0B}',
                        'f' => '\u{0C}',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'e' | 'E' => '\u{1B}',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        '$' => '$',
                        '`' => '`',
                        ' ' => ' ',
                        'u' => match parse_unicode(&mut chars) {
                            Ok(c3) => c3,
                            Err(_) => ' ',
                        },
                        _ => ' ',
                    });
                    continue;
                }
            };
        }
        res.push(c);
    }
    return res;
}

fn parse_unicode<I>(chars: &mut I) -> Result<char, String>
where
    I: Iterator<Item = (usize, char)>,
{
    match chars.next() {
        Some((_, '{')) => {}
        _ => {
            return Err("expected '{{' character in unicode escape".to_string());
        }
    }

    let unicode_seq: String = chars.take_while(|&(_, c)| c != '}').map(|(_, c)| c).collect();

    u32::from_str_radix(&unicode_seq, 16).map_err(|e| format!("could not parse {} as u32 hex: {}", unicode_seq, e)).and_then(|u| char::from_u32(u).ok_or_else(|| format!("could not parse {} as a unicode char", u)))
}
