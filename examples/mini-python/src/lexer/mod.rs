//! Mini Python 词法分析器

pub use self::token_type::PythonTokenType;
use gaia_types::{lexer::LexerState, reader::TokenStream, *};

pub mod token_type;

/// Python 词法分析器
pub struct PythonLexer<'input> {
    input: &'input str,
}

/// Python 词法分析器状态
pub struct PythonLexerState<'input> {
    state: LexerState<'input, PythonTokenType>,
    indent_stack: Vec<usize>, // 缩进栈
    at_line_start: bool,      // 是否在行首
}

impl<'input> PythonLexer<'input> {
    /// 创建一个新的 Python 词法分析器实例
    pub fn new(input: &'input str) -> Self {
        Self { input }
    }

    /// 对输入的 Python 代码进行词法分析
    pub fn tokenize(&mut self) -> GaiaDiagnostics<TokenStream<'input, PythonTokenType>> {
        let mut lexer_state =
            PythonLexerState { state: LexerState::new(self.input, None), indent_stack: vec![0], at_line_start: true };

        while let Some((mut offset, mut ch)) = lexer_state.state.peek() {
            // 如果在行首，先处理缩进
            if lexer_state.at_line_start && ch != '\n' && ch != '#' {
                self.handle_indentation(&mut lexer_state);
                // 重新获取当前字符，因为 handle_indentation 可能消耗了空白符
                if let Some((o, c)) = lexer_state.state.peek() {
                    offset = o;
                    ch = c;
                } else {
                    break;
                }
            }

            match ch {
                // 处理换行符
                '\n' => {
                    let (_, line, column) = lexer_state.state.mark_position();
                    lexer_state.state.add_token(PythonTokenType::Newline, offset, 1, line, column);
                    lexer_state.state.next_char();
                    lexer_state.at_line_start = true;
                }

                // 处理空白字符（不在行首时跳过）
                ch if ch.is_whitespace() && ch != '\n' => {
                    lexer_state.state.skip_whitespace(PythonTokenType::Whitespace);
                }

                // 处理注释
                '#' => {
                    lexer_state.state.skip_line_comment(PythonTokenType::Comment, "#");
                }

                // 处理字符串字面量
                '"' | '\'' => {
                    self.handle_string_literal(&mut lexer_state, ch);
                }

                // 处理数字
                ch if ch.is_ascii_digit() => {
                    self.handle_number(&mut lexer_state);
                }

                // 处理标识符和关键字
                ch if ch.is_alphabetic() || ch == '_' => {
                    self.handle_identifier(&mut lexer_state);
                }

                // 处理运算符
                '+' => self.handle_plus(&mut lexer_state),
                '-' => self.handle_minus(&mut lexer_state),
                '*' => self.handle_star(&mut lexer_state),
                '/' => self.handle_slash(&mut lexer_state),
                '%' => self.handle_percent(&mut lexer_state),
                '=' => self.handle_equal(&mut lexer_state),
                '!' => self.handle_bang(&mut lexer_state),
                '<' => self.handle_less(&mut lexer_state),
                '>' => self.handle_greater(&mut lexer_state),
                '~' => lexer_state.add_single_char_token(PythonTokenType::Tilde, '~'),
                '|' => lexer_state.add_single_char_token(PythonTokenType::Pipe, '|'),
                '^' => lexer_state.add_single_char_token(PythonTokenType::Caret, '^'),
                '&' => lexer_state.add_single_char_token(PythonTokenType::Ampersand, '&'),

                // 处理分隔符
                '(' => lexer_state.add_single_char_token(PythonTokenType::LeftParen, '('),
                ')' => lexer_state.add_single_char_token(PythonTokenType::RightParen, ')'),
                '[' => lexer_state.add_single_char_token(PythonTokenType::LeftBracket, '['),
                ']' => lexer_state.add_single_char_token(PythonTokenType::RightBracket, ']'),
                '{' => lexer_state.add_single_char_token(PythonTokenType::LeftBrace, '{'),
                '}' => lexer_state.add_single_char_token(PythonTokenType::RightBrace, '}'),
                ',' => lexer_state.add_single_char_token(PythonTokenType::Comma, ','),
                '.' => lexer_state.add_single_char_token(PythonTokenType::Dot, '.'),
                ':' => lexer_state.add_single_char_token(PythonTokenType::Colon, ':'),
                ';' => lexer_state.add_single_char_token(PythonTokenType::Semicolon, ';'),

                _ => {
                    // 跳过未知字符
                    lexer_state.state.next_char();
                }
            }
        }

        // 在文件结束时处理剩余的 DEDENT
        self.handle_end_of_file(&mut lexer_state);

        lexer_state.state.success()
    }

    /// 处理缩进
    fn handle_indentation(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        let mut indent_level = 0;

        // 计算当前行的缩进级别
        while let Some((_, ch)) = lexer_state.state.peek() {
            if ch == ' ' {
                indent_level += 1;
                lexer_state.state.next_char();
            }
            else if ch == '\t' {
                indent_level += 8; // 制表符按 8 个空格计算
                lexer_state.state.next_char();
            }
            else {
                break;
            }
        }

        lexer_state.at_line_start = false;

        // 检查是否是空行或注释行
        if let Some((_, ch)) = lexer_state.state.peek() {
            if ch == '\n' || ch == '#' {
                return; // 忽略空行和注释行的缩进
            }
        }

        let current_indent = *lexer_state.indent_stack.last().unwrap();

        if indent_level > current_indent {
            // 增加缩进
            lexer_state.indent_stack.push(indent_level);
            lexer_state.state.add_token(PythonTokenType::Indent, offset, indent_level, line, column);
        }
        else if indent_level < current_indent {
            // 减少缩进
            while let Some(&stack_indent) = lexer_state.indent_stack.last() {
                if stack_indent <= indent_level {
                    break;
                }
                lexer_state.indent_stack.pop();
                lexer_state.state.add_token(PythonTokenType::Dedent, offset, 0, line, column);
            }
        }
    }

    /// 处理 f-string 字面量
    fn handle_f_string_literal(
        &self,
        lexer_state: &mut PythonLexerState<'input>,
        start_offset: usize,
        start_line: u32,
        start_column: u32,
        quote_char: char,
    ) {
        lexer_state.state.next_char(); // 消费开始引号
        let mut length = (lexer_state.state.get_position().0 - start_offset) as usize;

        while let Some((_, ch)) = lexer_state.state.peek() {
            length += ch.len_utf8();
            lexer_state.state.next_char();
            if ch == quote_char {
                break;
            }
            if ch == '\\' {
                // 处理转义字符
                if let Some((_, _)) = lexer_state.state.peek() {
                    length += 1;
                    lexer_state.state.next_char();
                }
            }
        }

        lexer_state.state.add_token(PythonTokenType::String, start_offset, length, start_line, start_column);
        lexer_state.at_line_start = false;
    }

    /// 处理字符串字面量
    fn handle_string_literal(&self, lexer_state: &mut PythonLexerState<'input>, quote_char: char) {
        let (offset, line, column) = lexer_state.state.mark_position();
        let mut length = 1;
        lexer_state.state.next_char(); // 消费开始引号

        while let Some((_, ch)) = lexer_state.state.peek() {
            length += ch.len_utf8();
            lexer_state.state.next_char();
            if ch == quote_char {
                break;
            }
            if ch == '\\' {
                // 处理转义字符
                if let Some((_, _)) = lexer_state.state.peek() {
                    length += 1;
                    lexer_state.state.next_char();
                }
            }
        }

        lexer_state.state.add_token(PythonTokenType::String, offset, length, line, column);
        lexer_state.at_line_start = false;
    }

    /// 处理数字
    fn handle_number(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        let mut length = 0;
        let mut is_float = false;

        // 读取整数部分
        while let Some((_, ch)) = lexer_state.state.peek() {
            if ch.is_ascii_digit() {
                length += ch.len_utf8();
                lexer_state.state.next_char();
            }
            else {
                break;
            }
        }

        // 检查小数点
        {
            let rest = lexer_state.state.rest_text();
            if rest.starts_with('.') {
                if let Some(next_ch) = rest[1..].chars().next() {
                    if next_ch.is_ascii_digit() {
                        is_float = true;
                        length += 1;
                        lexer_state.state.next_char(); // 消费小数点

                        // 读取小数部分
                        while let Some((_, ch)) = lexer_state.state.peek() {
                            if ch.is_ascii_digit() {
                                length += ch.len_utf8();
                                lexer_state.state.next_char();
                            }
                            else {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let token_type = if is_float { PythonTokenType::Float } else { PythonTokenType::Integer };
        lexer_state.state.add_token(token_type, offset, length, line, column);
        lexer_state.at_line_start = false;
    }

    /// 处理标识符和关键字
    fn handle_identifier(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        let mut length = 0;

        while let Some((_, ch)) = lexer_state.state.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                length += ch.len_utf8();
                lexer_state.state.next_char();
            }
            else {
                break;
            }
        }

        // 获取标识符文本
        let identifier_text = &self.input[offset..offset + length];

        // 检查是否是 f-string 的前缀 (f 或 F)
        if (identifier_text == "f" || identifier_text == "F") && length == 1 {
            if let Some((_, quote_char)) = lexer_state.state.peek() {
                if quote_char == '"' || quote_char == '\'' {
                    // 这是一个 f-string
                    self.handle_f_string_literal(lexer_state, offset, line, column, quote_char);
                    return;
                }
            }
        }

        // 匹配 Python 关键字
        let token_type = match identifier_text {
            "def" => PythonTokenType::Def,
            "class" => PythonTokenType::Class,
            "if" => PythonTokenType::If,
            "elif" => PythonTokenType::Elif,
            "else" => PythonTokenType::Else,
            "for" => PythonTokenType::For,
            "while" => PythonTokenType::While,
            "in" => PythonTokenType::In,
            "return" => PythonTokenType::Return,
            "break" => PythonTokenType::Break,
            "continue" => PythonTokenType::Continue,
            "pass" => PythonTokenType::Pass,
            "import" => PythonTokenType::Import,
            "from" => PythonTokenType::From,
            "as" => PythonTokenType::As,
            "try" => PythonTokenType::Try,
            "except" => PythonTokenType::Except,
            "finally" => PythonTokenType::Finally,
            "raise" => PythonTokenType::Raise,
            "with" => PythonTokenType::With,
            "lambda" => PythonTokenType::Lambda,
            "and" => PythonTokenType::And,
            "or" => PythonTokenType::Or,
            "not" => PythonTokenType::Not,
            "is" => PythonTokenType::Is,
            "None" => PythonTokenType::None,
            "True" => PythonTokenType::True,
            "False" => PythonTokenType::False,
            _ => PythonTokenType::Identifier,
        };

        lexer_state.state.add_token(token_type, offset, length, line, column);
        lexer_state.at_line_start = false;
    }

    /// 处理加号和复合赋值
    fn handle_plus(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '+'

        if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::PlusEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Plus, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理减号和箭头
    fn handle_minus(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '-'

        if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::MinusEqual, offset, 2, line, column);
        }
        else if let Some((_, '>')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '>'
            lexer_state.state.add_token(PythonTokenType::Arrow, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Minus, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理星号
    fn handle_star(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '*'

        if let Some((_, '*')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费第二个 '*'
            if let Some((_, '=')) = lexer_state.state.peek() {
                lexer_state.state.next_char(); // 消费 '='
                lexer_state.state.add_token(PythonTokenType::DoubleStarEqual, offset, 3, line, column);
            }
            else {
                lexer_state.state.add_token(PythonTokenType::DoubleStar, offset, 2, line, column);
            }
        }
        else if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::StarEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Star, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理斜杠
    fn handle_slash(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '/'

        if let Some((_, '/')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费第二个 '/'
            if let Some((_, '=')) = lexer_state.state.peek() {
                lexer_state.state.next_char(); // 消费 '='
                lexer_state.state.add_token(PythonTokenType::DoubleSlashEqual, offset, 3, line, column);
            }
            else {
                lexer_state.state.add_token(PythonTokenType::DoubleSlash, offset, 2, line, column);
            }
        }
        else if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::SlashEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Slash, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理百分号
    fn handle_percent(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '%'
        if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::PercentEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Percent, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理等号
    fn handle_equal(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '='

        if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费第二个 '='
            lexer_state.state.add_token(PythonTokenType::EqualEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Equal, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理感叹号
    fn handle_bang(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '!'

        if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::BangEqual, offset, 2, line, column);
        }
        else {
            // Python 中单独的 '!' 不是有效的操作符，但我们仍然处理它
            lexer_state.state.next_char();
        }
        lexer_state.at_line_start = false;
    }

    /// 处理小于号
    fn handle_less(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '<'

        if let Some((_, '<')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费第二个 '<'
            lexer_state.state.add_token(PythonTokenType::LeftShift, offset, 2, line, column);
        }
        else if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::LessEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Less, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理大于号
    fn handle_greater(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();
        lexer_state.state.next_char(); // 消费 '>'

        if let Some((_, '>')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费第二个 '>'
            lexer_state.state.add_token(PythonTokenType::RightShift, offset, 2, line, column);
        }
        else if let Some((_, '=')) = lexer_state.state.peek() {
            lexer_state.state.next_char(); // 消费 '='
            lexer_state.state.add_token(PythonTokenType::GreaterEqual, offset, 2, line, column);
        }
        else {
            lexer_state.state.add_token(PythonTokenType::Greater, offset, 1, line, column);
        }
        lexer_state.at_line_start = false;
    }

    /// 处理文件结束
    fn handle_end_of_file(&self, lexer_state: &mut PythonLexerState<'input>) {
        let (offset, line, column) = lexer_state.state.mark_position();

        // 在文件结束时生成所有必要的 DEDENT token
        while lexer_state.indent_stack.len() > 1 {
            lexer_state.indent_stack.pop();
            lexer_state.state.add_token(PythonTokenType::Dedent, offset, 0, line, column);
        }
    }
}

// 将 add_single_char_token 改为 PythonLexerState 的方法，使用安全推进 API
impl<'input> PythonLexerState<'input> {
    fn add_single_char_token(&mut self, token_type: PythonTokenType, expected: char) {
        self.state.advance_by_char(token_type, expected);
        self.at_line_start = false;
    }
}
