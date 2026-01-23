//! Mini C 词法分析器

pub use self::token_type::CTokenType;
use crate::config::ReadConfig;
use gaia_types::{lexer::LexerState, reader::TokenStream, *};

mod token_type;

/// C 词法分析器
#[derive(Clone, Debug)]
pub struct CLexer<'config> {
    config: &'config ReadConfig,
}

impl<'config> CLexer<'config> {
    pub fn new(config: &'config ReadConfig) -> Self {
        Self { config }
    }

    /// 对输入的 C 代码进行词法分析
    pub fn tokenize<'input>(&self, input: &'input str) -> GaiaDiagnostics<TokenStream<'input, CTokenType>>
    where
        'config: 'input,
    {
        let mut state = LexerState::new(input, self.config.url.as_ref());

        while let Some((_offset, ch)) = state.peek() {
            match ch {
                // 处理空白字符
                ch if ch.is_whitespace() => {
                    state.skip_whitespace(CTokenType::Whitespace);
                }

                // 处理注释
                '/' => {
                    // 尝试跳过单行注释
                    if state.skip_line_comment(CTokenType::Comment, "//").is_none() {
                        // 尝试跳过块注释 /* ... */
                        let rest = state.rest_text();
                        if rest.starts_with("/*") {
                            let (start, line, column) = state.mark_position();
                            // 消耗 '/*'
                            state.next_char();
                            if let Some((_, '*')) = state.peek() {
                                state.next_char();
                            }

                            // 读取直到遇到 '*/'
                            let mut last_was_star = false;
                            while let Some((_, ch)) = state.peek() {
                                state.next_char();
                                if last_was_star && ch == '/' {
                                    break;
                                }
                                last_was_star = ch == '*';
                            }

                            // 计算长度并添加注释 token
                            if let Some((end_offset, _)) = state.peek() {
                                let length = end_offset - start;
                                state.add_token(CTokenType::Comment, start, length, line, column);
                            }
                            else {
                                // 到达 EOF，长度为已消耗长度
                                let length = state.rest_text().len();
                                state.add_token(CTokenType::Comment, start, length, line, column);
                            }
                        }
                        else {
                            // 不是注释，处理斜杠运算符
                            self.handle_slash(&mut state);
                        }
                    }
                }

                // 处理预处理器指令
                '#' => {
                    self.handle_preprocessor(&mut state, input);
                }

                // 处理字符串字面量
                '"' => {
                    state.read_string_literal(CTokenType::StringLiteral, '"');
                }

                // 处理字符字面量
                '\'' => {
                    state.read_string_literal(CTokenType::CharLiteral, '\'');
                }

                // 处理数字
                ch if ch.is_ascii_digit() => {
                    self.handle_number(&mut state);
                }

                // 处理标识符和关键字
                ch if ch.is_alphabetic() || ch == '_' => {
                    if let Some(token) = state.read_identifier(CTokenType::Identifier, |c| c.is_alphanumeric() || c == '_') {
                        // 获取标识符文本
                        let identifier_text = &input[token.position.offset..token.position.offset + token.position.length];

                        // 匹配关键字
                        let token_type = match identifier_text {
                            "auto" => CTokenType::Auto,
                            "break" => CTokenType::Break,
                            "case" => CTokenType::Case,
                            "char" => CTokenType::Char,
                            "const" => CTokenType::Const,
                            "continue" => CTokenType::Continue,
                            "default" => CTokenType::Default,
                            "do" => CTokenType::Do,
                            "double" => CTokenType::Double,
                            "else" => CTokenType::Else,
                            "enum" => CTokenType::Enum,
                            "extern" => CTokenType::Extern,
                            "float" => CTokenType::Float,
                            "for" => CTokenType::For,
                            "goto" => CTokenType::Goto,
                            "if" => CTokenType::If,
                            "int" => CTokenType::Int,
                            "long" => CTokenType::Long,
                            "register" => CTokenType::Register,
                            "return" => CTokenType::Return,
                            "short" => CTokenType::Short,
                            "signed" => CTokenType::Signed,
                            "sizeof" => CTokenType::Sizeof,
                            "static" => CTokenType::Static,
                            "struct" => CTokenType::Struct,
                            "switch" => CTokenType::Switch,
                            "typedef" => CTokenType::Typedef,
                            "union" => CTokenType::Union,
                            "unsigned" => CTokenType::Unsigned,
                            "void" => CTokenType::Void,
                            "volatile" => CTokenType::Volatile,
                            "while" => CTokenType::While,
                            _ => CTokenType::Identifier,
                        };

                        // 更新最后一个 token 的类型
                        state.update_last_token_type(token_type);
                    }
                }

                // 处理运算符
                '+' => self.handle_plus(&mut state),
                '-' => self.handle_minus(&mut state),
                '*' => self.handle_star(&mut state),
                '%' => self.handle_percent(&mut state),
                '=' => self.handle_equal(&mut state),
                '!' => self.handle_bang(&mut state),
                '<' => self.handle_less(&mut state),
                '>' => self.handle_greater(&mut state),
                '&' => self.handle_ampersand(&mut state),
                '|' => self.handle_pipe(&mut state),
                '^' => self.handle_caret(&mut state),
                '~' => self.add_single_char_token(&mut state, CTokenType::Tilde),
                '?' => self.add_single_char_token(&mut state, CTokenType::Question),

                // 处理分隔符
                '(' => self.add_single_char_token(&mut state, CTokenType::LeftParen),
                ')' => self.add_single_char_token(&mut state, CTokenType::RightParen),
                '[' => self.add_single_char_token(&mut state, CTokenType::LeftBracket),
                ']' => self.add_single_char_token(&mut state, CTokenType::RightBracket),
                '{' => self.add_single_char_token(&mut state, CTokenType::LeftBrace),
                '}' => self.add_single_char_token(&mut state, CTokenType::RightBrace),
                ',' => self.add_single_char_token(&mut state, CTokenType::Comma),
                '.' => self.add_single_char_token(&mut state, CTokenType::Dot),
                ':' => self.add_single_char_token(&mut state, CTokenType::Colon),
                ';' => self.add_single_char_token(&mut state, CTokenType::Semicolon),

                _ => {
                    // 跳过未知字符
                    state.next_char();
                }
            }
        }

        state.success()
    }

    fn add_single_char_token(&self, state: &mut LexerState<CTokenType>, token_type: CTokenType) {
        let (start, line, column) = state.mark_position();
        state.next_char();
        state.add_token(token_type, start, 1, line, column);
    }

    fn handle_preprocessor(&self, state: &mut LexerState<CTokenType>, input: &str) {
        let (start, line, column) = state.mark_position();
        state.next_char(); // 跳过 '#'

        // 跳过空白字符（不含换行）
        while let Some((_, ch)) = state.peek() {
            if ch.is_whitespace() && ch != '\n' {
                state.next_char();
            }
            else {
                break;
            }
        }

        // 读取预处理指令标识符
        let directive_start = if let Some((offset, _)) = state.peek() { offset } else { start };
        while let Some((_, ch)) = state.peek() {
            if ch.is_alphabetic() {
                state.next_char();
            }
            else {
                break;
            }
        }

        let directive_end = if let Some((offset, _)) = state.peek() { offset } else { directive_start };
        if directive_end > directive_start {
            let directive = &input[directive_start..directive_end];
            let token_type = match directive {
                "include" => CTokenType::Include,
                "define" => CTokenType::Define,
                _ => CTokenType::Hash,
            };

            // 消耗直到行尾
            while let Some((_, ch)) = state.peek() {
                if ch == '\n' {
                    break;
                }
                state.next_char();
            }

            // 重新计算长度，包含指令内容
            if let Some((end_offset, _)) = state.peek() {
                let length = end_offset - start;
                state.add_token(token_type, start, length, line, column);
            } else {
                let length = input.len() - start;
                state.add_token(token_type, start, length, line, column);
            }
        }
        else {
            state.add_token(CTokenType::Hash, start, 1, line, column);
        }
    }

    fn handle_number(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        let mut length = 0;
        let mut is_float = false;

        // 读取整数部分
        while let Some((_, ch)) = state.peek() {
            if ch.is_ascii_digit() {
                state.next_char();
                length += 1;
            }
            else {
                break;
            }
        }

        // 检查是否有小数点
        if let Some((_, '.')) = state.peek() {
            state.next_char();
            length += 1;
            is_float = true;

            // 读取小数部分
            while let Some((_, ch)) = state.peek() {
                if ch.is_ascii_digit() {
                    state.next_char();
                    length += 1;
                }
                else {
                    break;
                }
            }
        }

        let token_type = if is_float { CTokenType::FloatLiteral } else { CTokenType::IntegerLiteral };

        state.add_token(token_type, start, length, line, column);
    }

    fn handle_plus(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '+' => {
                    state.next_char();
                    state.add_token(CTokenType::Increment, start, 2, line, column);
                }
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::PlusEqual, start, 2, line, column);
                }
                _ => {
                    state.add_token(CTokenType::Plus, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Plus, start, 1, line, column);
        }
    }

    fn handle_minus(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '-' => {
                    state.next_char();
                    state.add_token(CTokenType::Decrement, start, 2, line, column);
                }
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::MinusEqual, start, 2, line, column);
                }
                '>' => {
                    state.next_char();
                    state.add_token(CTokenType::Arrow, start, 2, line, column);
                }
                _ => {
                    state.add_token(CTokenType::Minus, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Minus, start, 1, line, column);
        }
    }

    fn handle_star(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::StarEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Star, start, 1, line, column);
        }
    }

    fn handle_slash(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::SlashEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Slash, start, 1, line, column);
        }
    }

    fn handle_percent(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::PercentEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Percent, start, 1, line, column);
        }
    }

    fn handle_equal(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::EqualEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Equal, start, 1, line, column);
        }
    }

    fn handle_bang(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::BangEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Bang, start, 1, line, column);
        }
    }

    fn handle_less(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::LessEqual, start, 2, line, column);
                }
                '<' => {
                    state.next_char();
                    if let Some((_, '=')) = state.peek() {
                        state.next_char();
                        state.add_token(CTokenType::LeftShiftEqual, start, 3, line, column);
                    }
                    else {
                        state.add_token(CTokenType::LeftShift, start, 2, line, column);
                    }
                }
                _ => {
                    state.add_token(CTokenType::Less, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Less, start, 1, line, column);
        }
    }

    fn handle_greater(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::GreaterEqual, start, 2, line, column);
                }
                '>' => {
                    state.next_char();
                    if let Some((_, '=')) = state.peek() {
                        state.next_char();
                        state.add_token(CTokenType::RightShiftEqual, start, 3, line, column);
                    }
                    else {
                        state.add_token(CTokenType::RightShift, start, 2, line, column);
                    }
                }
                _ => {
                    state.add_token(CTokenType::Greater, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Greater, start, 1, line, column);
        }
    }

    fn handle_ampersand(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '&' => {
                    state.next_char();
                    state.add_token(CTokenType::LogicalAnd, start, 2, line, column);
                }
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::AmpersandEqual, start, 2, line, column);
                }
                _ => {
                    state.add_token(CTokenType::Ampersand, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Ampersand, start, 1, line, column);
        }
    }

    fn handle_pipe(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, next_ch)) = state.peek() {
            match next_ch {
                '|' => {
                    state.next_char();
                    state.add_token(CTokenType::LogicalOr, start, 2, line, column);
                }
                '=' => {
                    state.next_char();
                    state.add_token(CTokenType::PipeEqual, start, 2, line, column);
                }
                _ => {
                    state.add_token(CTokenType::Pipe, start, 1, line, column);
                }
            }
        }
        else {
            state.add_token(CTokenType::Pipe, start, 1, line, column);
        }
    }

    fn handle_caret(&self, state: &mut LexerState<CTokenType>) {
        let (start, line, column) = state.mark_position();
        state.next_char();

        if let Some((_, '=')) = state.peek() {
            state.next_char();
            state.add_token(CTokenType::CaretEqual, start, 2, line, column);
        }
        else {
            state.add_token(CTokenType::Caret, start, 1, line, column);
        }
    }
}
