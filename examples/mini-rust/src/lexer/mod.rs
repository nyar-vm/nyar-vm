//! Mini Rust 词法分析器

pub use self::token_type::RustTokenType;
use gaia_types::{lexer::LexerState, reader::TokenStream, *};

mod token_type;

/// 词法分析器
pub struct RustLexer<'input> {
    input: &'input str,
}

impl<'input> RustLexer<'input> {
    /// 创建一个新的 Rust 词法分析器实例
    pub fn new(input: &'input str) -> Self {
        Self { input }
    }

    /// 对输入的 Rust 代码进行词法分析
    ///
    /// # 返回值
    ///
    /// 返回包含 token 流的 `GaiaDiagnostics`，如果分析成功则包含 `TokenStream<RustTokenType>`
    pub fn tokenize(&mut self) -> GaiaDiagnostics<TokenStream<'input, RustTokenType>> {
        let mut state = LexerState::new(self.input, None);

        while let Some((offset, ch)) = state.peek() {
            match ch {
                // 处理空白字符
                ch if ch.is_whitespace() => {
                    state.skip_whitespace(RustTokenType::Whitespace);
                }

                // 处理注释
                '/' => {
                    if state.skip_line_comment(RustTokenType::Comment, "//").is_none() {
                        // 如果不是注释，则作为普通符号处理
                        let (_, line, column) = state.mark_position();
                        state.add_token(RustTokenType::Slash, offset, 1, line, column);
                        state.next_char(); // 消费字符
                    }
                }

                // 处理字符串字面量
                '"' => {
                    let (_, start_line, start_column) = state.mark_position();
                    let mut length = 1; // 开始的引号
                    state.next_char(); // 消费开始的引号

                    // 读取字符串内容直到结束引号
                    while let Some((_, ch)) = state.peek() {
                        length += ch.len_utf8();
                        state.next_char();
                        if ch == '"' {
                            break;
                        }
                    }

                    state.add_token(RustTokenType::StringLiteral, offset, length, start_line, start_column);
                }

                // 处理字符字面量
                '\'' => {
                    let (_, start_line, start_column) = state.mark_position();
                    let mut length = 1; // 开始的单引号
                    state.next_char(); // 消费开始的单引号

                    if let Some((_, ch)) = state.peek() {
                        length += ch.len_utf8();
                        state.next_char();
                        if let Some((_, '\'')) = state.peek() {
                            length += 1;
                            state.next_char(); // 消费结束的单引号
                        }
                    }

                    state.add_token(RustTokenType::CharLiteral, offset, length, start_line, start_column);
                }

                // 处理数字
                ch if ch.is_ascii_digit() => {
                    let (_, start_line, start_column) = state.mark_position();
                    let mut length = ch.len_utf8();
                    state.next_char(); // 消费第一个数字字符

                    // 读取连续的数字字符
                    while let Some((_, ch)) = state.peek() {
                        if ch.is_ascii_digit() {
                            length += ch.len_utf8();
                            state.next_char();
                        }
                        else {
                            break;
                        }
                    }

                    state.add_token(RustTokenType::Integer, offset, length, start_line, start_column);
                }

                // 处理标识符和关键字
                ch if ch.is_alphabetic() || ch == '_' => {
                    let (_, start_line, start_column) = state.mark_position();
                    let mut length = ch.len_utf8();
                    state.next_char(); // 消费第一个字符

                    // 读取连续的标识符字符
                    while let Some((_, ch)) = state.peek() {
                        if ch.is_alphanumeric() || ch == '_' {
                            length += ch.len_utf8();
                            state.next_char();
                        }
                        else {
                            break;
                        }
                    }

                    // 获取标识符文本
                    let identifier_text = &self.input[offset..offset + length];

                    // 匹配关键字
                    let token_type = match identifier_text {
                        "fn" => RustTokenType::Fn,
                        "let" => RustTokenType::Let,
                        "mut" => RustTokenType::Mut,
                        "return" => RustTokenType::Return,
                        "if" => RustTokenType::If,
                        "else" => RustTokenType::Else,
                        "while" => RustTokenType::While,
                        "for" => RustTokenType::For,
                        "in" => RustTokenType::In,
                        "struct" => RustTokenType::Struct,
                        "true" => RustTokenType::True,
                        "false" => RustTokenType::False,
                        _ => RustTokenType::Identifier,
                    };

                    state.add_token(token_type, offset, length, start_line, start_column);
                }

                // 处理运算符和符号
                '+' => {
                    let (_, line, column) = state.mark_position();
                    state.add_token(RustTokenType::Plus, offset, 1, line, column);
                    state.next_char(); // 消费字符
                }
                '-' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '-'
                    if let Some((_, '>')) = state.peek() {
                        state.next_char(); // 消费 '>'
                        state.add_token(RustTokenType::Arrow, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Minus, offset, 1, line, column);
                    }
                }
                '*' => {
                    let (_, line, column) = state.mark_position();
                    state.add_token(RustTokenType::Star, offset, 1, line, column);
                    state.next_char(); // 消费字符
                }
                '=' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '='
                    if let Some((_, '=')) = state.peek() {
                        state.next_char(); // 消费第二个 '='
                        state.add_token(RustTokenType::EqualEqual, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Equal, offset, 1, line, column);
                    }
                }
                '!' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '!'
                    if let Some((_, '=')) = state.peek() {
                        state.next_char(); // 消费 '='
                        state.add_token(RustTokenType::NotEqual, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Bang, offset, 1, line, column);
                    }
                }
                '&' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '&'
                    if let Some((_, '&')) = state.peek() {
                        state.next_char(); // 消费第二个 '&'
                        state.add_token(RustTokenType::AndAnd, offset, 2, line, column);
                    }
                    else {
                        // 目前不支持单个 &
                        state.add_token(RustTokenType::Identifier, offset, 1, line, column);
                    }
                }
                '|' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '|'
                    if let Some((_, '|')) = state.peek() {
                        state.next_char(); // 消费第二个 '|'
                        state.add_token(RustTokenType::OrOr, offset, 2, line, column);
                    }
                    else {
                        // 目前不支持单个 |
                        state.add_token(RustTokenType::Identifier, offset, 1, line, column);
                    }
                }
                '<' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '<'
                    if let Some((_, '=')) = state.peek() {
                        state.next_char(); // 消费 '='
                        state.add_token(RustTokenType::LessEqual, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Less, offset, 1, line, column);
                    }
                }
                '>' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '>'
                    if let Some((_, '=')) = state.peek() {
                        state.next_char(); // 消费 '='
                        state.add_token(RustTokenType::GreaterEqual, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Greater, offset, 1, line, column);
                    }
                }

                // 处理分隔符
                '{' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '{'
                    state.add_token(RustTokenType::LeftBrace, offset, 1, line, column);
                }
                '}' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '}'
                    state.add_token(RustTokenType::RightBrace, offset, 1, line, column);
                }
                '(' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '('
                    state.add_token(RustTokenType::LeftParen, offset, 1, line, column);
                }
                ')' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 ')'
                    state.add_token(RustTokenType::RightParen, offset, 1, line, column);
                }
                '[' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '['
                    state.add_token(RustTokenType::LeftBracket, offset, 1, line, column);
                }
                ']' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 ']'
                    state.add_token(RustTokenType::RightBracket, offset, 1, line, column);
                }
                ':' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 ':'
                    state.add_token(RustTokenType::Colon, offset, 1, line, column);
                }
                ';' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 ';'
                    state.add_token(RustTokenType::Semicolon, offset, 1, line, column);
                }
                '.' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 '.'
                    if let Some((_, '.')) = state.peek() {
                        state.next_char(); // 消费第二个 '.'
                        state.add_token(RustTokenType::DotDot, offset, 2, line, column);
                    }
                    else {
                        state.add_token(RustTokenType::Dot, offset, 1, line, column);
                    }
                }
                ',' => {
                    let (_, line, column) = state.mark_position();
                    state.next_char(); // 消费 ','
                    state.add_token(RustTokenType::Comma, offset, 1, line, column);
                }

                _ => {
                    // 忽略其他字符 - 字符已经被消费了
                }
            }
        }

        state.success()
    }
}
