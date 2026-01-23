//! Mini C 语法分析器

use crate::{ast::*, lexer::CTokenType};
use gaia_types::{
    reader::{Token, TokenStream},
    GaiaError, SourceLocation,
};

/// C 语法分析器
pub struct CParser<'input> {
    raw: &'input str,
    tokens: Vec<Token<CTokenType>>,
    current: usize,
}

type ParseResult<T> = Result<T, GaiaError>;

impl<'input> CParser<'input> {
    /// 创建新的解析器
    pub fn new(token_stream: TokenStream<'input, CTokenType>) -> Self {
        let raw = token_stream.raw;
        let mut parser = Self { raw, tokens: token_stream.tokens.into_inner(), current: 0 };
        // 跳过开头的空白字符
        while parser.current < parser.tokens.len() && parser.tokens[parser.current].token_type == CTokenType::Whitespace {
            parser.current += 1;
        }
        parser
    }

    /// 解析程序
    pub fn parse(&mut self) -> ParseResult<Program> {
        let mut declarations = Vec::new();

        while !self.is_at_end() {
            // 跳过预处理指令
            if matches!(self.peek().token_type, CTokenType::Include | CTokenType::Define | CTokenType::Hash) {
                self.advance();
                continue;
            }
            declarations.push(self.parse_declaration()?);
        }

        Ok(Program { declarations })
    }

    /// 解析声明
    fn parse_declaration(&mut self) -> ParseResult<Declaration> {
        // 检查类型说明符
        if self.check_type_specifier() {
            self.parse_variable_or_function_declaration()
        }
        else {
            Err(self.error("Expected declaration"))
        }
    }

    /// 检查是否是类型说明符
    fn check_type_specifier(&self) -> bool {
        matches!(self.peek().token_type, CTokenType::Int | CTokenType::Float | CTokenType::Char | CTokenType::Void)
    }

    /// 解析变量或函数声明
    fn parse_variable_or_function_declaration(&mut self) -> ParseResult<Declaration> {
        let type_spec = self.parse_type_specifier()?;
        let name = self.consume_identifier("Expected identifier")?;

        if self.match_token(&CTokenType::LeftParen) {
            // 函数声明
            self.parse_function_declaration(type_spec, name)
        }
        else {
            // 变量声明
            self.parse_variable_declaration(type_spec, name)
        }
    }

    /// 解析类型说明符
    fn parse_type_specifier(&mut self) -> ParseResult<Type> {
        let base_type = match &self.peek().token_type {
            CTokenType::Int => {
                self.advance();
                Type::Basic(BasicType::Int)
            }
            CTokenType::Float => {
                self.advance();
                Type::Basic(BasicType::Float)
            }
            CTokenType::Char => {
                self.advance();
                Type::Basic(BasicType::Char)
            }
            CTokenType::Void => {
                self.advance();
                Type::Basic(BasicType::Void)
            }
            _ => return Err(self.error("Expected type specifier")),
        };

        // 处理指针类型
        let mut result_type = base_type;
        while self.match_token(&CTokenType::Star) {
            result_type = Type::Pointer(Box::new(result_type));
        }

        Ok(result_type)
    }

    /// 解析函数声明
    fn parse_function_declaration(&mut self, return_type: Type, name: String) -> ParseResult<Declaration> {
        let mut parameters = Vec::new();

        if !self.check(&CTokenType::RightParen) {
            loop {
                let param_type = self.parse_type_specifier()?;
                let param_name = self.consume_identifier("Expected parameter name")?;
                parameters.push(Parameter { type_: param_type, name: Some(param_name) });

                if !self.match_token(&CTokenType::Comma) {
                    break;
                }
            }
        }

        self.consume(&CTokenType::RightParen, "Expected ')' after parameters")?;

        let body = if self.match_token(&CTokenType::LeftBrace) {
            Some(self.parse_compound_statement()?)
        }
        else {
            self.consume(&CTokenType::Semicolon, "Expected ';' after function declaration")?;
            None
        };

        Ok(Declaration::Function { return_type, name, parameters, body })
    }

    /// 解析变量声明
    fn parse_variable_declaration(&mut self, var_type: Type, name: String) -> ParseResult<Declaration> {
        let initializer = if self.match_token(&CTokenType::Equal) { Some(self.parse_expression()?) } else { None };

        self.consume(&CTokenType::Semicolon, "Expected ';' after variable declaration")?;

        Ok(Declaration::Variable { type_: var_type, name, initializer })
    }

    /// 解析复合语句
    fn parse_compound_statement(&mut self) -> ParseResult<CompoundStatement> {
        let mut statements = Vec::new();

        while !self.check(&CTokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }

        self.consume(&CTokenType::RightBrace, "Expected '}' after compound statement")?;

        Ok(CompoundStatement { statements })
    }

    /// 解析语句
    fn parse_statement(&mut self) -> ParseResult<Statement> {
        match &self.peek().token_type {
            CTokenType::If => self.parse_if_statement(),
            CTokenType::While => self.parse_while_statement(),
            CTokenType::For => self.parse_for_statement(),
            CTokenType::Return => self.parse_return_statement(),
            CTokenType::LeftBrace => {
                self.advance();
                Ok(Statement::Compound(self.parse_compound_statement()?))
            }
            _ => {
                if self.check_type_specifier() {
                    // 局部变量声明
                    let decl = self.parse_declaration()?;
                    Ok(Statement::Declaration(decl))
                }
                else {
                    // 表达式语句
                    let expr = self.parse_expression()?;
                    self.consume(&CTokenType::Semicolon, "Expected ';' after expression")?;
                    Ok(Statement::Expression(Some(expr)))
                }
            }
        }
    }

    /// 解析 if 语句
    fn parse_if_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&CTokenType::If, "Expected 'if'")?;
        self.consume(&CTokenType::LeftParen, "Expected '(' after 'if'")?;
        let condition = self.parse_expression()?;
        self.consume(&CTokenType::RightParen, "Expected ')' after if condition")?;

        let then_stmt = Box::new(self.parse_statement()?);
        let else_stmt = if self.match_token(&CTokenType::Else) { Some(Box::new(self.parse_statement()?)) } else { None };

        Ok(Statement::If { condition, then_stmt, else_stmt })
    }

    /// 解析 while 语句
    fn parse_while_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&CTokenType::While, "Expected 'while'")?;
        self.consume(&CTokenType::LeftParen, "Expected '(' after 'while'")?;
        let condition = self.parse_expression()?;
        self.consume(&CTokenType::RightParen, "Expected ')' after while condition")?;

        let body = Box::new(self.parse_statement()?);

        Ok(Statement::While { condition, body })
    }

    /// 解析 for 语句
    fn parse_for_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&CTokenType::For, "Expected 'for'")?;
        self.consume(&CTokenType::LeftParen, "Expected '(' after 'for'")?;

        let init = if self.match_token(&CTokenType::Semicolon) { None } else { Some(self.parse_expression()?) };

        let condition = if self.check(&CTokenType::Semicolon) { None } else { Some(self.parse_expression()?) };
        self.consume(&CTokenType::Semicolon, "Expected ';' after for condition")?;

        let update = if self.check(&CTokenType::RightParen) { None } else { Some(self.parse_expression()?) };
        self.consume(&CTokenType::RightParen, "Expected ')' after for clauses")?;

        let body = Box::new(self.parse_statement()?);

        Ok(Statement::For { init, condition, update, body })
    }

    /// 解析 return 语句
    fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        self.consume(&CTokenType::Return, "Expected 'return'")?;

        let value = if self.check(&CTokenType::Semicolon) { None } else { Some(self.parse_expression()?) };

        self.consume(&CTokenType::Semicolon, "Expected ';' after return statement")?;

        Ok(Statement::Return(value))
    }

    /// 解析表达式
    fn parse_expression(&mut self) -> ParseResult<Expression> {
        self.parse_assignment()
    }

    /// 解析赋值表达式
    fn parse_assignment(&mut self) -> ParseResult<Expression> {
        let expr = self.parse_logical_or()?;

        if self.match_token(&CTokenType::Equal) {
            let value = self.parse_assignment()?;
            return Ok(Expression::Assignment {
                left: Box::new(expr),
                operator: AssignmentOperator::Assign,
                right: Box::new(value),
            });
        }

        Ok(expr)
    }

    /// 解析逻辑或表达式
    fn parse_logical_or(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_logical_and()?;

        while self.match_token(&CTokenType::LogicalOr) {
            let operator = BinaryOperator::LogicalOr;
            let right = self.parse_logical_and()?;
            expr = Expression::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 解析逻辑与表达式
    fn parse_logical_and(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_equality()?;

        while self.match_token(&CTokenType::LogicalAnd) {
            let operator = BinaryOperator::LogicalAnd;
            let right = self.parse_equality()?;
            expr = Expression::Binary { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 解析相等性表达式
    fn parse_equality(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_relational()?;

        while let Some(op) = self.match_equality_operator() {
            let right = self.parse_relational()?;
            expr = Expression::Binary { left: Box::new(expr), operator: op, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 匹配相等性操作符
    fn match_equality_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            CTokenType::EqualEqual => {
                self.advance();
                Some(BinaryOperator::Equal)
            }
            CTokenType::BangEqual => {
                self.advance();
                Some(BinaryOperator::NotEqual)
            }
            _ => None,
        }
    }

    /// 解析关系表达式
    fn parse_relational(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_additive()?;

        while let Some(op) = self.match_relational_operator() {
            let right = self.parse_additive()?;
            expr = Expression::Binary { left: Box::new(expr), operator: op, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 匹配关系操作符
    fn match_relational_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            CTokenType::Less => {
                self.advance();
                Some(BinaryOperator::Less)
            }
            CTokenType::LessEqual => {
                self.advance();
                Some(BinaryOperator::LessEqual)
            }
            CTokenType::Greater => {
                self.advance();
                Some(BinaryOperator::Greater)
            }
            CTokenType::GreaterEqual => {
                self.advance();
                Some(BinaryOperator::GreaterEqual)
            }
            _ => None,
        }
    }

    /// 解析加法表达式
    fn parse_additive(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_multiplicative()?;

        while let Some(op) = self.match_additive_operator() {
            let right = self.parse_multiplicative()?;
            expr = Expression::Binary { left: Box::new(expr), operator: op, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 匹配加法操作符
    fn match_additive_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            CTokenType::Plus => {
                self.advance();
                Some(BinaryOperator::Add)
            }
            CTokenType::Minus => {
                self.advance();
                Some(BinaryOperator::Subtract)
            }
            _ => None,
        }
    }

    /// 解析乘法表达式
    fn parse_multiplicative(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_unary()?;

        while let Some(op) = self.match_multiplicative_operator() {
            let right = self.parse_unary()?;
            expr = Expression::Binary { left: Box::new(expr), operator: op, right: Box::new(right) };
        }

        Ok(expr)
    }

    /// 匹配乘法操作符
    fn match_multiplicative_operator(&mut self) -> Option<BinaryOperator> {
        match &self.peek().token_type {
            CTokenType::Star => {
                self.advance();
                Some(BinaryOperator::Multiply)
            }
            CTokenType::Slash => {
                self.advance();
                Some(BinaryOperator::Divide)
            }
            CTokenType::Percent => {
                self.advance();
                Some(BinaryOperator::Modulo)
            }
            _ => None,
        }
    }

    /// 解析一元表达式
    fn parse_unary(&mut self) -> ParseResult<Expression> {
        match &self.peek().token_type {
            CTokenType::Bang => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::Unary { operator: UnaryOperator::LogicalNot, operand: Box::new(expr) })
            }
            CTokenType::Minus => {
                self.advance();
                let expr = self.parse_unary()?;
                Ok(Expression::Unary { operator: UnaryOperator::Minus, operand: Box::new(expr) })
            }
            _ => self.parse_postfix(),
        }
    }

    /// 解析后缀表达式
    fn parse_postfix(&mut self) -> ParseResult<Expression> {
        let mut expr = self.parse_primary()?;

        loop {
            match &self.peek().token_type {
                CTokenType::LeftParen => {
                    self.advance();
                    let mut arguments = Vec::new();

                    if !self.check(&CTokenType::RightParen) {
                        loop {
                            arguments.push(self.parse_expression()?);
                            if !self.match_token(&CTokenType::Comma) {
                                break;
                            }
                        }
                    }

                    self.consume(&CTokenType::RightParen, "Expected ')' after arguments")?;

                    expr = Expression::Call { function: Box::new(expr), arguments };
                }
                CTokenType::LeftBracket => {
                    self.advance();
                    let index = self.parse_expression()?;
                    self.consume(&CTokenType::RightBracket, "Expected ']' after array index")?;

                    expr = Expression::ArrayAccess { array: Box::new(expr), index: Box::new(index) };
                }
                _ => break,
            }
        }

        Ok(expr)
    }

    /// 解析基本表达式
    fn parse_primary(&mut self) -> ParseResult<Expression> {
        match &self.peek().token_type {
            CTokenType::IntegerLiteral => {
                let token = self.advance();
                let value = self.raw[token.position.offset..token.position.offset + token.position.length]
                    .parse::<i64>()
                    .map_err(|_| self.error("Invalid integer literal"))?;
                Ok(Expression::Literal(Literal::Integer(value)))
            }
            CTokenType::FloatLiteral => {
                let token = self.advance();
                let value = self.raw[token.position.offset..token.position.offset + token.position.length]
                    .parse::<f64>()
                    .map_err(|_| self.error("Invalid float literal"))?;
                Ok(Expression::Literal(Literal::Float(value)))
            }
            CTokenType::StringLiteral => {
                let token = self.advance();
                let value = self.raw[token.position.offset + 1..token.position.offset + token.position.length - 1].to_string();
                Ok(Expression::Literal(Literal::String(value)))
            }
            CTokenType::CharLiteral => {
                let token = self.advance();
                let value = self.raw[token.position.offset + 1..token.position.offset + token.position.length - 1]
                    .chars()
                    .next()
                    .ok_or_else(|| self.error("Invalid character literal"))?;
                Ok(Expression::Literal(Literal::Character(value)))
            }
            CTokenType::Identifier => {
                let name = self.consume_identifier("Expected identifier")?;
                Ok(Expression::Identifier(name))
            }
            CTokenType::LeftParen => {
                self.advance();
                let expr = self.parse_expression()?;
                self.consume(&CTokenType::RightParen, "Expected ')' after expression")?;
                Ok(expr)
            }
            _ => Err(self.error("Expected expression")),
        }
    }

    /// 工具方法：匹配 token
    fn match_token(&mut self, token_type: &CTokenType) -> bool {
        if self.check(token_type) {
            self.advance();
            true
        }
        else {
            false
        }
    }

    /// 工具方法：检查当前 token
    fn check(&self, token_type: &CTokenType) -> bool {
        if self.is_at_end() {
            false
        }
        else {
            &self.peek().token_type == token_type
        }
    }

    /// 工具方法：前进到下一个 token（跳过空白字符）
    fn advance(&mut self) -> Token<CTokenType> {
        if !self.is_at_end() {
            // 跳过当前token
            self.current += 1;
            // 跳过空白字符
            while self.current < self.tokens.len() && self.tokens[self.current].token_type == CTokenType::Whitespace {
                self.current += 1;
            }
        }
        self.previous().clone()
    }

    /// 工具方法：检查是否到达末尾
    fn is_at_end(&self) -> bool {
        self.current >= self.tokens.len() || self.peek().token_type == CTokenType::Eof
    }

    /// 工具方法：查看当前 token（跳过空白字符）
    fn peek(&self) -> &Token<CTokenType> {
        let mut pos = self.current;
        while pos < self.tokens.len() {
            let token = &self.tokens[pos];
            if token.token_type != CTokenType::Whitespace {
                return token;
            }
            pos += 1;
        }
        // 如果没有找到非空白token，返回最后一个token（通常是EOF）
        &self.tokens[self.tokens.len() - 1]
    }

    /// 工具方法：查看前一个 token
    fn previous(&self) -> &Token<CTokenType> {
        let mut pos = self.current.saturating_sub(1);
        // 向前查找最近的非空白token
        while pos > 0 && self.tokens[pos].token_type == CTokenType::Whitespace {
            pos = pos.saturating_sub(1);
        }
        &self.tokens[pos]
    }

    /// 工具方法：消费指定类型的 token
    fn consume(&mut self, token_type: &CTokenType, message: &str) -> ParseResult<()> {
        if self.check(token_type) {
            self.advance();
            Ok(())
        }
        else {
            Err(self.error(message))
        }
    }

    /// 工具方法：消费标识符
    fn consume_identifier(&mut self, message: &str) -> ParseResult<String> {
        if self.check(&CTokenType::Identifier) {
            let token = self.advance();
            Ok(self.raw[token.position.offset..token.position.offset + token.position.length].to_string())
        }
        else {
            Err(self.error(message))
        }
    }

    /// 工具方法：创建错误
    fn error(&self, message: &str) -> GaiaError {
        let token = self.peek();
        GaiaError::syntax_error(message, SourceLocation { line: token.position.line, column: token.position.column, url: None })
    }
}

#[cfg(test)]
mod tests {
}
