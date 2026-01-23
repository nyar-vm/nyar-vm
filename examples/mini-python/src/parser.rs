//! Mini Python 语法分析器

use crate::{ast::*, lexer::PythonTokenType};
use gaia_types::reader::{Token, TokenStream};

/// Python 语法分析器
pub struct PythonParser {
    raw: String,
    tokens: Vec<Token<PythonTokenType>>,
    current: usize,
}

/// 解析错误
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    SyntaxError { message: String, line: u32, column: u32 },
    LexError(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::SyntaxError { message, line, column } => {
                write!(f, "Parse error at line {}, column {}: {}", line, column, message)
            }
            ParseError::LexError(msg) => {
                write!(f, "Lexer error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ParseError {}

type ParseResult<T> = Result<T, ParseError>;

impl PythonParser {
    /// 创建新的解析器
    pub fn new(token_stream: TokenStream<PythonTokenType>) -> Self {
        let raw = token_stream.raw.to_string();
        let tokens = token_stream
            .tokens
            .into_inner()
            .into_iter()
            .filter(|t| t.token_type != PythonTokenType::Whitespace && t.token_type != PythonTokenType::Comment)
            .collect();
        Self { raw, tokens, current: 0 }
    }

    /// 解析程序
    pub fn parse(&mut self) -> ParseResult<Program> {
        let mut statements = Vec::new();

        // 跳过开头的空白符和换行符
        self.skip_whitespace_and_newlines();

        while !self.is_at_end() {
            if self.check(&PythonTokenType::Newline) || self.check(&PythonTokenType::Whitespace) {
                self.advance();
                continue;
            }

            let stmt = self.parse_statement()?;
            statements.push(stmt);

            // 跳过语句后的空白符和换行符
            self.skip_whitespace_and_newlines();
        }

        Ok(Program { statements })
    }

    /// 解析语句
    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.peek().token_type {
            PythonTokenType::Def => self.parse_function_def(),
            PythonTokenType::Class => self.parse_class_def(),
            PythonTokenType::If => self.parse_if_statement(),
            PythonTokenType::For => self.parse_for_statement(),
            PythonTokenType::While => self.parse_while_statement(),
            PythonTokenType::Return => self.parse_return_statement(),
            PythonTokenType::Break => {
                self.advance();
                Ok(Statement::Break)
            }
            PythonTokenType::Continue => {
                self.advance();
                Ok(Statement::Continue)
            }
            PythonTokenType::Pass => {
                self.advance();
                Ok(Statement::Pass)
            }
            PythonTokenType::Import => self.parse_import_statement(),
            PythonTokenType::From => self.parse_from_import_statement(),
            PythonTokenType::Try => self.parse_try_statement(),
            PythonTokenType::Raise => self.parse_raise_statement(),
            PythonTokenType::With => self.parse_with_statement(),
            _ => self.parse_expression_or_assignment(),
        }
    }

    /// 解析函数定义
    fn parse_function_def(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Def, "Expected 'def'")?;
        self.skip_whitespace();

        let name = self.consume_identifier("Expected function name")?;

        self.consume(&PythonTokenType::LeftParen, "Expected '(' after function name")?;

        let mut parameters = Vec::new();
        if !self.check(&PythonTokenType::RightParen) {
            loop {
                self.skip_whitespace();
                let param_name = self.consume_identifier("Expected parameter name")?;
                let mut annotation = None;
                let mut default = None;

                // 类型注解
                if self.match_token(&PythonTokenType::Colon) {
                    self.skip_whitespace();
                    annotation = Some(self.parse_type()?);
                }

                // 默认值
                if self.match_token(&PythonTokenType::Equal) {
                    self.skip_whitespace();
                    default = Some(self.parse_expression()?);
                }

                parameters.push(Parameter { name: param_name, annotation, default });

                if !self.match_token(&PythonTokenType::Comma) {
                    break;
                }
            }
        }

        self.skip_whitespace();
        self.consume(&PythonTokenType::RightParen, "Expected ')' after parameters")?;

        // 返回类型注解
        let return_type = if self.match_token(&PythonTokenType::Arrow) { Some(self.parse_type()?) } else { None };

        self.consume(&PythonTokenType::Colon, "Expected ':' after function signature")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(Statement::FunctionDef { name, parameters, return_type, body })
    }

    /// 解析类定义
    fn parse_class_def(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Class, "Expected 'class'")?;

        let name = self.consume_identifier("Expected class name")?;

        let mut bases = Vec::new();
        if self.match_token(&PythonTokenType::LeftParen) {
            if !self.check(&PythonTokenType::RightParen) {
                loop {
                    bases.push(self.parse_expression()?);
                    if !self.match_token(&PythonTokenType::Comma) {
                        break;
                    }
                }
            }
            self.consume(&PythonTokenType::RightParen, "Expected ')' after base classes")?;
        }

        self.consume(&PythonTokenType::Colon, "Expected ':' after class signature")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(Statement::ClassDef { name, bases, body })
    }

    /// 解析 if 语句
    fn parse_if_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::If, "Expected 'if'")?;

        let test = self.parse_expression()?;
        self.consume(&PythonTokenType::Colon, "Expected ':' after if condition")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        let mut orelse = Vec::new();
        if self.match_token(&PythonTokenType::Elif) {
            // 递归处理 elif
            orelse.push(self.parse_if_statement()?);
        }
        else if self.match_token(&PythonTokenType::Else) {
            self.consume(&PythonTokenType::Colon, "Expected ':' after else")?;
            self.skip_newlines();
            orelse = self.parse_block()?;
        }

        Ok(Statement::If { test, body, orelse })
    }

    /// 解析 for 语句
    fn parse_for_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::For, "Expected 'for'")?;

        let target = self.parse_expression()?;
        self.consume(&PythonTokenType::In, "Expected 'in' after for target")?;
        let iter = self.parse_expression()?;

        self.consume(&PythonTokenType::Colon, "Expected ':' after for clause")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        let orelse = if self.match_token(&PythonTokenType::Else) {
            self.consume(&PythonTokenType::Colon, "Expected ':' after else")?;
            self.skip_newlines();
            self.parse_block()?
        }
        else {
            Vec::new()
        };

        Ok(Statement::For { target, iter, body, orelse })
    }

    /// 解析 while 语句
    fn parse_while_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::While, "Expected 'while'")?;

        let test = self.parse_expression()?;
        self.consume(&PythonTokenType::Colon, "Expected ':' after while condition")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        let orelse = if self.match_token(&PythonTokenType::Else) {
            self.consume(&PythonTokenType::Colon, "Expected ':' after else")?;
            self.skip_newlines();
            self.parse_block()?
        }
        else {
            Vec::new()
        };

        Ok(Statement::While { test, body, orelse })
    }

    /// 解析 return 语句
    fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Return, "Expected 'return'")?;

        let value =
            if self.check(&PythonTokenType::Newline) || self.is_at_end() { None } else { Some(self.parse_expression()?) };

        Ok(Statement::Return(value))
    }

    /// 解析 import 语句
    fn parse_import_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Import, "Expected 'import'")?;

        let mut names = Vec::new();
        loop {
            let name = self.consume_identifier("Expected module name")?;
            let asname = if self.match_token(&PythonTokenType::As) {
                Some(self.consume_identifier("Expected alias name")?)
            }
            else {
                None
            };

            names.push(ImportName { name, asname });

            if !self.match_token(&PythonTokenType::Comma) {
                break;
            }
        }

        Ok(Statement::Import { names })
    }

    /// 解析 from import 语句
    fn parse_from_import_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::From, "Expected 'from'")?;

        let module = if self.check(&PythonTokenType::Identifier) {
            Some(self.consume_identifier("Expected module name")?)
        }
        else {
            None
        };

        self.consume(&PythonTokenType::Import, "Expected 'import' after from clause")?;

        let mut names = Vec::new();
        if self.match_token(&PythonTokenType::Star) {
            names.push(ImportName { name: "*".to_string(), asname: None });
        }
        else {
            loop {
                let name = self.consume_identifier("Expected import name")?;
                let asname = if self.match_token(&PythonTokenType::As) {
                    Some(self.consume_identifier("Expected alias name")?)
                }
                else {
                    None
                };

                names.push(ImportName { name, asname });

                if !self.match_token(&PythonTokenType::Comma) {
                    break;
                }
            }
        }

        Ok(Statement::ImportFrom { module, names })
    }

    /// 解析 try 语句
    fn parse_try_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Try, "Expected 'try'")?;
        self.consume(&PythonTokenType::Colon, "Expected ':' after try")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        let mut handlers = Vec::new();
        while self.match_token(&PythonTokenType::Except) {
            let type_ = if !self.check(&PythonTokenType::Colon) { Some(self.parse_expression()?) } else { None };

            let name = if self.match_token(&PythonTokenType::As) {
                Some(self.consume_identifier("Expected exception name")?)
            }
            else {
                None
            };

            self.consume(&PythonTokenType::Colon, "Expected ':' after except clause")?;
            self.skip_newlines();

            let handler_body = self.parse_block()?;

            handlers.push(ExceptHandler { type_, name, body: handler_body });
        }

        let orelse = if self.match_token(&PythonTokenType::Else) {
            self.consume(&PythonTokenType::Colon, "Expected ':' after else")?;
            self.skip_newlines();
            self.parse_block()?
        }
        else {
            Vec::new()
        };

        let finalbody = if self.match_token(&PythonTokenType::Finally) {
            self.consume(&PythonTokenType::Colon, "Expected ':' after finally")?;
            self.skip_newlines();
            self.parse_block()?
        }
        else {
            Vec::new()
        };

        Ok(Statement::Try { body, handlers, orelse, finalbody })
    }

    /// 解析 raise 语句
    fn parse_raise_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::Raise, "Expected 'raise'")?;

        let exc = if self.check(&PythonTokenType::Newline) || self.is_at_end() { None } else { Some(self.parse_expression()?) };

        let cause = if self.match_token(&PythonTokenType::From) { Some(self.parse_expression()?) } else { None };

        Ok(Statement::Raise { exc, cause })
    }

    /// 解析 with 语句
    fn parse_with_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&PythonTokenType::With, "Expected 'with'")?;

        let mut items = Vec::new();
        loop {
            let context_expr = self.parse_expression()?;
            let optional_vars = if self.match_token(&PythonTokenType::As) { Some(self.parse_expression()?) } else { None };

            items.push(WithItem { context_expr, optional_vars });

            if !self.match_token(&PythonTokenType::Comma) {
                break;
            }
        }

        self.consume(&PythonTokenType::Colon, "Expected ':' after with clause")?;
        self.skip_newlines();

        let body = self.parse_block()?;

        Ok(Statement::With { items, body })
    }

    /// 解析表达式或赋值语句
    fn parse_expression_or_assignment(&mut self) -> ParseResult<Statement> {
        self.skip_whitespace();
        let expr = self.parse_expression()?;

        self.skip_whitespace();
        // 检查是否是赋值语句
        if self.match_token(&PythonTokenType::Equal) {
            self.skip_whitespace();
            let value = self.parse_expression()?;
            Ok(Statement::Assignment { target: expr, value })
        }
        else if let Some(op) = self.match_augmented_assignment() {
            self.skip_whitespace();
            let value = self.parse_expression()?;
            Ok(Statement::AugmentedAssignment { target: expr, operator: op, value })
        }
        else {
            Ok(Statement::Expression(expr))
        }
    }

    /// 解析代码块
    fn parse_block(&mut self) -> ParseResult<Vec<Statement>> {
        self.consume(&PythonTokenType::Indent, "Expected indentation")?;

        let mut statements = Vec::new();

        while !self.check(&PythonTokenType::Dedent) && !self.is_at_end() {
            if self.check(&PythonTokenType::Newline) {
                self.advance();
                continue;
            }

            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }

        self.consume(&PythonTokenType::Dedent, "Expected dedentation")?;

        Ok(statements)
    }

    /// 解析表达式
    fn parse_expression(&mut self) -> ParseResult<Expression> {
        self.parse_or()
    }

    /// 解析 or 表达式
    fn parse_or(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_and()?;

        let mut values = vec![expr];
        while self.match_token(&PythonTokenType::Or) {
            values.push(self.parse_and()?);
        }

        if values.len() > 1 {
            Ok(Expression::BoolOp { operator: BoolOperator::Or, values })
        }
        else {
            Ok(values.into_iter().next().unwrap())
        }
    }

    /// 解析 and 表达式
    fn parse_and(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_not()?;

        let mut values = vec![expr];
        while self.match_token(&PythonTokenType::And) {
            values.push(self.parse_not()?);
        }

        if values.len() > 1 {
            Ok(Expression::BoolOp { operator: BoolOperator::And, values })
        }
        else {
            Ok(values.into_iter().next().unwrap())
        }
    }

    /// 解析 not 表达式
    fn parse_not(&mut self) -> ParseResult<Expression> {
        if self.match_token(&PythonTokenType::Not) {
            let operand = self.parse_not()?;
            Ok(Expression::UnaryOp { operator: UnaryOperator::Not, operand: Box::new(operand) })
        }
        else {
            self.parse_comparison()
        }
    }

    /// 解析比较表达式
    fn parse_comparison(&mut self) -> ParseResult<Expression> {
        let left = self.parse_bitwise_or()?;

        let mut ops = Vec::new();
        let mut comparators = Vec::new();

        while let Some(op) = self.match_comparison_operator() {
            ops.push(op);
            comparators.push(self.parse_bitwise_or()?);
        }

        if !ops.is_empty() {
            Ok(Expression::Compare { left: Box::new(left), ops, comparators })
        }
        else {
            Ok(left)
        }
    }

    /// 解析位或表达式
    fn parse_bitwise_or(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_bitwise_xor()?;

        while self.match_token(&PythonTokenType::Pipe) {
            let right = self.parse_bitwise_xor()?;
            left = Expression::BinaryOp { left: Box::new(left), operator: BinaryOperator::BitOr, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析位异或表达式
    fn parse_bitwise_xor(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_bitwise_and()?;

        while self.match_token(&PythonTokenType::Caret) {
            let right = self.parse_bitwise_and()?;
            left = Expression::BinaryOp { left: Box::new(left), operator: BinaryOperator::BitXor, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析位与表达式
    fn parse_bitwise_and(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_shift()?;

        while self.match_token(&PythonTokenType::Ampersand) {
            let right = self.parse_shift()?;
            left = Expression::BinaryOp { left: Box::new(left), operator: BinaryOperator::BitAnd, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析位移表达式
    fn parse_shift(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_term()?;

        while let Some(op) = self.match_shift_operator() {
            let right = self.parse_term()?;
            left = Expression::BinaryOp { left: Box::new(left), operator: op, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析项表达式
    fn parse_term(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_factor()?;

        while let Some(op) = self.match_term_operator() {
            let right = self.parse_factor()?;
            left = Expression::BinaryOp { left: Box::new(left), operator: op, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析因子表达式
    fn parse_factor(&mut self) -> ParseResult<Expression> {
        if let Some(op) = self.match_unary_operator() {
            let operand = self.parse_factor()?;
            Ok(Expression::UnaryOp { operator: op, operand: Box::new(operand) })
        }
        else {
            self.parse_power()
        }
    }

    /// 解析幂表达式
    fn parse_power(&mut self) -> ParseResult<Expression> {
        let mut left = self.parse_atom()?;

        if self.match_token(&PythonTokenType::DoubleStar) {
            let right = self.parse_factor()?; // 右结合
            left = Expression::BinaryOp { left: Box::new(left), operator: BinaryOperator::Pow, right: Box::new(right) };
        }

        Ok(left)
    }

    /// 解析原子表达式
    fn parse_atom(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&PythonTokenType::LeftParen) {
                // 函数调用
                let mut args = Vec::new();
                let keywords = Vec::new();

                if !self.check(&PythonTokenType::RightParen) {
                    loop {
                        // 简化处理，只支持位置参数
                        args.push(self.parse_expression()?);

                        if !self.match_token(&PythonTokenType::Comma) {
                            break;
                        }
                    }
                }

                self.consume(&PythonTokenType::RightParen, "Expected ')' after arguments")?;

                expr = Expression::Call { func: Box::new(expr), args, keywords };
            }
            else if self.match_token(&PythonTokenType::LeftBracket) {
                // 下标访问
                let slice = self.parse_expression()?;
                self.consume(&PythonTokenType::RightBracket, "Expected ']' after subscript")?;

                expr = Expression::Subscript { value: Box::new(expr), slice: Box::new(slice) };
            }
            else if self.match_token(&PythonTokenType::Dot) {
                // 属性访问
                let attr = self.consume_identifier("Expected attribute name")?;
                expr = Expression::Attribute { value: Box::new(expr), attr };
            }
            else {
                break;
            }
        }

        Ok(expr)
    }

    /// 解析基本表达式
    fn parse_primary(&mut self) -> ParseResult<Expression> {
        self.skip_whitespace();
        match &self.peek().token_type {
            PythonTokenType::True => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(true)))
            }
            PythonTokenType::False => {
                self.advance();
                Ok(Expression::Literal(Literal::Boolean(false)))
            }
            PythonTokenType::None => {
                self.advance();
                Ok(Expression::Literal(Literal::None))
            }
            PythonTokenType::Integer => {
                let token = self.advance().clone();
                let value = self.get_text(&token).parse::<i64>().map_err(|_| self.error("Invalid integer literal"))?;
                Ok(Expression::Literal(Literal::Integer(value)))
            }
            PythonTokenType::Float => {
                let token = self.advance().clone();
                let value = self.get_text(&token).parse::<f64>().map_err(|_| self.error("Invalid float literal"))?;
                Ok(Expression::Literal(Literal::Float(value)))
            }
            PythonTokenType::String => {
                let token = self.advance().clone();
                Ok(Expression::Literal(Literal::String(self.get_text(&token).to_string())))
            }
            PythonTokenType::Identifier => {
                let token = self.advance().clone();
                Ok(Expression::Name(self.get_text(&token).to_string()))
            }
            PythonTokenType::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(&PythonTokenType::RightParen, "Expected ')' after expression")?;
                Ok(expr)
            }
            PythonTokenType::LeftBracket => {
                self.advance();
                let mut elts = Vec::new();

                if !self.check(&PythonTokenType::RightBracket) {
                    loop {
                        elts.push(self.parse_expression()?);
                        if !self.match_token(&PythonTokenType::Comma) {
                            break;
                        }
                    }
                }

                self.consume(&PythonTokenType::RightBracket, "Expected ']' after list elements")?;
                Ok(Expression::List { elts })
            }
            PythonTokenType::LeftBrace => {
                self.advance();
                let mut keys = Vec::new();
                let mut values = Vec::new();

                if !self.check(&PythonTokenType::RightBrace) {
                    loop {
                        let key = self.parse_expression()?;
                        self.consume(&PythonTokenType::Colon, "Expected ':' after dict key")?;
                        let value = self.parse_expression()?;

                        keys.push(Some(key));
                        values.push(value);

                        if !self.match_token(&PythonTokenType::Comma) {
                            break;
                        }
                    }
                }

                self.consume(&PythonTokenType::RightBrace, "Expected '}' after dict elements")?;
                Ok(Expression::Dict { keys, values })
            }
            _ => Err(self.error("Expected expression")),
        }
    }

    /// 解析类型注解
    fn parse_type(&mut self) -> ParseResult<Type> {
        self.skip_whitespace();
        let name = self.consume_identifier("Expected type name")?;

        if self.match_token(&PythonTokenType::LeftBracket) {
            let mut args = Vec::new();

            if !self.check(&PythonTokenType::RightBracket) {
                loop {
                    args.push(self.parse_type()?);
                    if !self.match_token(&PythonTokenType::Comma) {
                        break;
                    }
                }
            }

            self.consume(&PythonTokenType::RightBracket, "Expected ']' after type arguments")?;

            Ok(Type::Generic { name, args })
        }
        else {
            Ok(Type::Name(name))
        }
    }

    // 辅助方法

    /// 匹配复合赋值运算符
    fn match_augmented_assignment(&mut self) -> Option<AugmentedOperator> {
        match &self.peek().token_type {
            PythonTokenType::PlusEqual => {
                self.advance();
                Some(AugmentedOperator::Add)
            }
            PythonTokenType::MinusEqual => {
                self.advance();
                Some(AugmentedOperator::Sub)
            }
            PythonTokenType::StarEqual => {
                self.advance();
                Some(AugmentedOperator::Mult)
            }
            PythonTokenType::SlashEqual => {
                self.advance();
                Some(AugmentedOperator::Div)
            }
            PythonTokenType::DoubleSlashEqual => {
                self.advance();
                Some(AugmentedOperator::FloorDiv)
            }
            PythonTokenType::PercentEqual => {
                self.advance();
                Some(AugmentedOperator::Mod)
            }
            PythonTokenType::DoubleStarEqual => {
                self.advance();
                Some(AugmentedOperator::Pow)
            }
            _ => None,
        }
    }

    /// 匹配比较运算符
    fn match_comparison_operator(&mut self) -> Option<CompareOperator> {
        match &self.peek().token_type {
            PythonTokenType::EqualEqual => {
                self.advance();
                Some(CompareOperator::Eq)
            }
            PythonTokenType::BangEqual => {
                self.advance();
                Some(CompareOperator::NotEq)
            }
            PythonTokenType::Less => {
                self.advance();
                Some(CompareOperator::Lt)
            }
            PythonTokenType::LessEqual => {
                self.advance();
                Some(CompareOperator::LtE)
            }
            PythonTokenType::Greater => {
                self.advance();
                Some(CompareOperator::Gt)
            }
            PythonTokenType::GreaterEqual => {
                self.advance();
                Some(CompareOperator::GtE)
            }
            PythonTokenType::Is => {
                self.advance();
                if self.match_token(&PythonTokenType::Not) {
                    Some(CompareOperator::IsNot)
                }
                else {
                    Some(CompareOperator::Is)
                }
            }
            PythonTokenType::In => {
                self.advance();
                Some(CompareOperator::In)
            }
            PythonTokenType::Not => {
                if self.peek_next().map(|t| &t.token_type) == Some(&PythonTokenType::In) {
                    self.advance(); // not
                    self.advance(); // in
                    Some(CompareOperator::NotIn)
                }
                else {
                    None
                }
            }
            _ => None,
        }
    }

    /// 匹配位移运算符
    fn match_shift_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            PythonTokenType::LeftShift => {
                self.advance();
                Some(BinaryOperator::LShift)
            }
            PythonTokenType::RightShift => {
                self.advance();
                Some(BinaryOperator::RShift)
            }
            _ => None,
        }
    }

    /// 匹配项运算符
    fn match_term_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            PythonTokenType::Plus => {
                self.advance();
                Some(BinaryOperator::Add)
            }
            PythonTokenType::Minus => {
                self.advance();
                Some(BinaryOperator::Sub)
            }
            _ => None,
        }
    }

    /// 匹配一元运算符
    fn match_unary_operator(&mut self) -> Option<UnaryOperator> {
        match &self.peek().token_type {
            PythonTokenType::Plus => {
                self.advance();
                Some(UnaryOperator::UAdd)
            }
            PythonTokenType::Minus => {
                self.advance();
                Some(UnaryOperator::USub)
            }
            PythonTokenType::Tilde => {
                self.advance();
                Some(UnaryOperator::Invert)
            }
            _ => None,
        }
    }

    /// 跳过换行符
    fn skip_newlines(&mut self) {
        while self.match_token(&PythonTokenType::Newline) {
            // 继续跳过
        }
    }

    /// 跳过空白符
    fn skip_whitespace(&mut self) {
        while self.match_token(&PythonTokenType::Whitespace) {
            // 继续跳过
        }
    }

    /// 跳过空白符和换行符
    fn skip_whitespace_and_newlines(&mut self) {
        loop {
            if self.match_token(&PythonTokenType::Whitespace) || self.match_token(&PythonTokenType::Newline) {
                continue;
            }
            break;
        }
    }

    /// 匹配并消费指定的 token
    fn match_token(&mut self, token_type: &PythonTokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        }
        else {
            false
        }
    }

    /// 检查当前 token 是否匹配指定类型
    fn check(&self, token_type: &PythonTokenType) -> bool {
        if self.is_at_end() {
            false
        }
        else {
            &self.peek().token_type == token_type
        }
    }

    /// 前进到下一个 token
    fn advance(&mut self) -> &Token<PythonTokenType> {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    /// 是否到达末尾
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len() || self.peek().token_type == PythonTokenType::Eof
    }

    /// 查看当前 token
    fn peek(&self) -> &Token<PythonTokenType> {
        if self.current < self.tokens.len() {
            &self.tokens[self.current]
        }
        else {
            // 返回最后一个 token（应为 EOF）
            &self.tokens[self.tokens.len().saturating_sub(1)]
        }
    }

    /// 查看下一个 token
    fn peek_next(&self) -> Option<&Token<PythonTokenType>> {
        if self.current + 1 < self.tokens.len() {
            Some(&self.tokens[self.current + 1])
        }
        else {
            None
        }
    }

    /// 查看前一个 token
    fn previous(&self) -> &Token<PythonTokenType> {
        &self.tokens[self.current - 1]
    }

    /// 消费指定类型的 token
    fn consume(&mut self, token_type: &PythonTokenType, message: &str) -> ParseResult<()> {
        if self.check(token_type) {
            self.advance();
            Ok(())
        }
        else {
            Err(self.error(message))
        }
    }

    /// 消费标识符 token
    fn consume_identifier(&mut self, message: &str) -> ParseResult<String> {
        if self.check(&PythonTokenType::Identifier) {
            let token = self.advance().clone();
            Ok(self.get_text(&token).to_string())
        }
        else {
            Err(self.error(message))
        }
    }

    /// 创建解析错误
    fn error(&self, message: &str) -> ParseError {
        let token = self.peek();
        ParseError::SyntaxError { message: message.to_string(), line: token.position.line, column: token.position.column }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::PythonLexer;

    #[test]
    fn test_parse_simple_function() {
        let code = r#"
def hello():
    print("Hello, World!")
"#;
        let mut lexer = PythonLexer::new(code);
        let token_stream = lexer.tokenize().result.unwrap();
        let mut parser = PythonParser::new(token_stream);

        let program = parser.parse().unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::FunctionDef { name, parameters, body, .. } => {
                assert_eq!(name, "hello");
                assert_eq!(parameters.len(), 0);
                assert_eq!(body.len(), 1);
            }
            _ => panic!("Expected function definition"),
        }
    }

    #[test]
    fn test_parse_function_with_parameters() {
        let code = r#"
def greet(name: str, age: int = 25):
    print(f"Hello, {name}! You are {age} years old.")
"#;
        let mut lexer = PythonLexer::new(code);
        let token_stream = lexer.tokenize().result.unwrap();
        let mut parser = PythonParser::new(token_stream);

        let program = parser.parse().unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::FunctionDef { name, parameters, .. } => {
                assert_eq!(name, "greet");
                assert_eq!(parameters.len(), 2);
                assert_eq!(parameters[0].name, "name");
                assert_eq!(parameters[1].name, "age");
                assert!(parameters[1].default.is_some());
            }
            _ => panic!("Expected function definition"),
        }
    }

    #[test]
    fn test_parse_assignment() {
        let code = "x = 42";

        let mut lexer = PythonLexer::new(code);
        let token_stream = lexer.tokenize().result.unwrap();
        let mut parser = PythonParser::new(token_stream);

        let program = parser.parse().unwrap();
        assert_eq!(program.statements.len(), 1);

        match &program.statements[0] {
            Statement::Assignment { target, value } => {
                match target {
                    Expression::Name(name) => assert_eq!(name, "x"),
                    _ => panic!("Expected name expression"),
                }
                match value {
                    Expression::Literal(Literal::Integer(n)) => assert_eq!(*n, 42),
                    _ => panic!("Expected integer literal"),
                }
            }
            _ => panic!("Expected assignment statement"),
        }
    }
}

impl PythonParser {
    /// 根据 token 的位置从原始源码中提取文本切片
    fn get_text<'a>(&'a self, token: &Token<PythonTokenType>) -> &'a str {
        let start = token.position.offset;
        let end = start + token.position.length;
        &self.raw[start..end]
    }
}
