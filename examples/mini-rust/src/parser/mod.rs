//! Mini Rust 语法分析器

use crate::{ast::*, lexer::RustTokenType};
use gaia_types::{reader::TokenStream, *};

/// 语法分析器
pub struct Parser<'input> {
    token_stream: TokenStream<'input, RustTokenType>,
    current_index: usize,
}

impl<'input> Parser<'input> {
    pub fn new(token_stream: TokenStream<'input, RustTokenType>) -> Self {
        Self { token_stream, current_index: 0 }
    }

    // 获取当前“有效”token：跳过空白与注释，但保留换行
    fn current_token(&self) -> &RustTokenType {
        self.peek_token(0)
    }

    fn peek_token(&self, skip: usize) -> &RustTokenType {
        let tokens = self.token_stream.tokens.get_ref();
        let mut idx = self.current_index;
        let mut valid_count = 0;

        while let Some(tok) = tokens.get(idx) {
            match tok.token_type {
                RustTokenType::Whitespace | RustTokenType::Comment => {
                    idx += 1;
                    continue;
                }
                _ => {
                    if valid_count == skip {
                        return &tok.token_type;
                    }
                    valid_count += 1;
                    idx += 1;
                }
            }
        }
        &RustTokenType::Eof
    }

    fn get_token_text(&self, index: usize) -> String {
        if let Some(token) = self.token_stream.tokens.get_ref().get(index) {
            match self.token_stream.get_text(token) {
                Ok(s) => s.to_string(),
                Err(_) => String::new(),
            }
        }
        else {
            String::new()
        }
    }

    // 获取当前有效 token 的源码文本
    fn current_token_text(&self) -> String {
        let tokens = self.token_stream.tokens.get_ref();
        let mut idx = self.current_index;
        while let Some(tok) = tokens.get(idx) {
            match tok.token_type {
                RustTokenType::Whitespace | RustTokenType::Comment => {
                    idx += 1;
                    continue;
                }
                _ => return self.get_token_text(idx),
            }
        }
        String::new()
    }

    fn advance(&mut self) -> Result<()> {
        let tokens = self.token_stream.tokens.get_ref();
        let mut idx = self.current_index;
        // 跳过当前索引起的空白/注释，定位到当前有效 token
        while idx < tokens.len() {
            match tokens[idx].token_type {
                RustTokenType::Whitespace | RustTokenType::Comment => idx += 1,
                _ => break,
            }
        }
        // 消费当前有效 token（如果存在），并将索引推进到其后一个位置
        if idx < tokens.len() {
            self.current_index = idx + 1;
        }
        else {
            self.current_index = idx;
        }
        Ok(())
    }

    fn expect(&mut self, expected: RustTokenType) -> Result<()> {
        if std::mem::discriminant(self.current_token()) == std::mem::discriminant(&expected) {
            self.advance()
        }
        else {
            Err(GaiaError::syntax_error(
                format!("期望 {:?}, 但得到 {:?}", expected, self.current_token()),
                SourceLocation::default(),
            ))
        }
    }

    fn skip_newlines(&mut self) -> Result<()> {
        while *self.current_token() == RustTokenType::Newline {
            self.advance()?;
        }
        Ok(())
    }

    pub fn parse_program(&mut self) -> Result<Program> {
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        self.skip_newlines()?;

        while *self.current_token() != RustTokenType::Eof {
            match self.current_token() {
                RustTokenType::Fn => {
                    functions.push(self.parse_function()?);
                }
                RustTokenType::Struct => {
                    structs.push(self.parse_struct_definition()?);
                }
                _ => {
                    return Err(GaiaError::syntax_error(
                        format!("期望 'fn' 或 'struct', 但得到 {:?}", self.current_token()),
                        SourceLocation::default(),
                    ))
                }
            }
            self.skip_newlines()?;
        }

        Ok(Program { name: "main".to_string(), functions, structs })
    }

    fn parse_struct_definition(&mut self) -> Result<StructDefinition> {
        self.expect(RustTokenType::Struct)?;

        let name = match self.current_token() {
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;
                name
            }
            _ => return Err(GaiaError::syntax_error("期望结构体名称".to_string(), SourceLocation::default())),
        };

        self.expect(RustTokenType::LeftBrace)?;
        self.skip_newlines()?;

        let mut fields = Vec::new();
        while *self.current_token() != RustTokenType::RightBrace {
            let field_name = match self.current_token() {
                RustTokenType::Identifier => {
                    let name = self.current_token_text();
                    self.advance()?;
                    name
                }
                _ => return Err(GaiaError::syntax_error("期望字段名".to_string(), SourceLocation::default())),
            };

            self.expect(RustTokenType::Colon)?;
            let field_type = self.parse_type()?;
            fields.push(FieldDefinition { name: field_name, field_type });

            if *self.current_token() == RustTokenType::Comma {
                self.advance()?;
                self.skip_newlines()?;
            }
            else if *self.current_token() != RustTokenType::RightBrace {
                return Err(GaiaError::syntax_error("期望 ',' 或 '}'".to_string(), SourceLocation::default()));
            }
        }

        self.expect(RustTokenType::RightBrace)?;

        Ok(StructDefinition { name, fields })
    }

    fn parse_function(&mut self) -> Result<Function> {
        self.expect(RustTokenType::Fn)?;

        let name = match self.current_token() {
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;
                name
            }
            _ => return Err(GaiaError::syntax_error("期望函数名".to_string(), SourceLocation::default())),
        };

        self.expect(RustTokenType::LeftParen)?;

        let mut parameters = Vec::new();
        while *self.current_token() != RustTokenType::RightParen {
            parameters.push(self.parse_parameter()?);

            if *self.current_token() == RustTokenType::Comma {
                self.advance()?;
            }
            else if *self.current_token() != RustTokenType::RightParen {
                return Err(GaiaError::syntax_error("期望 ',' 或 ')'".to_string(), SourceLocation::default()));
            }
        }

        self.expect(RustTokenType::RightParen)?;

        // 可选的返回类型
        let return_type = if *self.current_token() == RustTokenType::Arrow {
            self.advance()?;
            Some(self.parse_type()?)
        }
        else {
            None
        };

        self.skip_newlines()?;
        let body = self.parse_block()?;

        Ok(Function { name, parameters, return_type, body })
    }

    fn parse_parameter(&mut self) -> Result<Parameter> {
        let is_mutable = if *self.current_token() == RustTokenType::Mut {
            self.advance()?;
            true
        }
        else {
            false
        };

        let name = match self.current_token() {
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;
                name
            }
            _ => return Err(GaiaError::syntax_error("期望参数名".to_string(), SourceLocation::default())),
        };

        self.expect(RustTokenType::Colon)?;
        let param_type = self.parse_type()?;

        Ok(Parameter { name, param_type, is_mutable })
    }

    fn parse_type(&mut self) -> Result<Type> {
        match self.current_token() {
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;
                match name.as_str() {
                    "i32" => Ok(Type::I32),
                    "i64" => Ok(Type::I64),
                    "f32" => Ok(Type::F32),
                    "f64" => Ok(Type::F64),
                    "String" => Ok(Type::String),
                    "bool" => Ok(Type::Bool),
                    _ => Ok(Type::Custom(name)),
                }
            }
            RustTokenType::LeftParen => {
                // 处理 unit 类型 ()
                self.advance()?; // 跳过 (
                self.expect(RustTokenType::RightParen)?; // 期望 )
                Ok(Type::Unit)
            }
            RustTokenType::LeftBracket => {
                // 处理数组类型 [T]
                self.advance()?; // 跳过 [
                let element_type = self.parse_type()?;
                self.expect(RustTokenType::RightBracket)?; // 期望 ]
                Ok(Type::Array(Box::new(element_type)))
            }
            _ => Err(GaiaError::syntax_error(
                format!("期望类型名, 但得到 {:?}", self.current_token()),
                SourceLocation::default(),
            )),
        }
    }

    fn parse_block(&mut self) -> Result<Block> {
        self.expect(RustTokenType::LeftBrace)?;
        self.skip_newlines()?;

        let mut statements = Vec::new();

        while *self.current_token() != RustTokenType::RightBrace && *self.current_token() != RustTokenType::Eof {
            statements.push(self.parse_statement()?);
            self.skip_newlines()?;
        }

        self.expect(RustTokenType::RightBrace)?;

        Ok(Block { statements })
    }

    fn parse_statement(&mut self) -> Result<Statement> {
        match self.current_token() {
            RustTokenType::Let => self.parse_variable_declaration(),
            RustTokenType::Return => self.parse_return_statement(),
            RustTokenType::If => self.parse_if_statement(),
            RustTokenType::While => self.parse_while_statement(),
            RustTokenType::For => self.parse_for_statement(),
            _ => {
                let expr = self.parse_expression()?;

                // 检查是否是赋值语句
                if *self.current_token() == RustTokenType::Equal {
                    self.advance()?; // 消费 '='
                    let value = self.parse_expression()?;
                    if *self.current_token() == RustTokenType::Semicolon {
                        self.advance()?;
                    }
                    Ok(Statement::Assignment { target: expr, value })
                }
                else {
                    if *self.current_token() == RustTokenType::Semicolon {
                        self.advance()?;
                    }
                    Ok(Statement::Expression(expr))
                }
            }
        }
    }

    fn parse_assignment(&mut self) -> Result<Statement> {
        // 这个方法现在已经不再直接从 parse_statement 调用，因为逻辑已经合并
        Err(GaiaError::custom_error("Deprecated: parse_assignment is now integrated into parse_statement"))
    }

    fn parse_if_statement(&mut self) -> Result<Statement> {
        self.expect(RustTokenType::If)?;
        let condition = self.parse_expression()?;
        let then_branch = self.parse_block()?;

        let else_branch = if *self.current_token() == RustTokenType::Else {
            self.advance()?;
            if *self.current_token() == RustTokenType::If {
                Some(Block { statements: vec![self.parse_if_statement()?] })
            }
            else {
                Some(self.parse_block()?)
            }
        }
        else {
            None
        };

        Ok(Statement::If { condition, then_branch, else_branch })
    }

    fn parse_while_statement(&mut self) -> Result<Statement> {
        self.expect(RustTokenType::While)?;
        let condition = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::While { condition, body })
    }

    fn parse_for_statement(&mut self) -> Result<Statement> {
        self.expect(RustTokenType::For)?;
        let var_name = self.current_token_text();
        self.expect(RustTokenType::Identifier)?;
        self.expect(RustTokenType::In)?;
        let iterable = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Statement::For { var_name, iterable, body })
    }

    fn parse_variable_declaration(&mut self) -> Result<Statement> {
        self.expect(RustTokenType::Let)?;

        let is_mutable = if *self.current_token() == RustTokenType::Mut {
            self.advance()?;
            true
        }
        else {
            false
        };

        let name = match self.current_token() {
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;
                name
            }
            _ => return Err(GaiaError::syntax_error("期望变量名".to_string(), SourceLocation::default())),
        };

        let var_type = if *self.current_token() == RustTokenType::Colon {
            self.advance()?;
            Some(self.parse_type()?)
        }
        else {
            None
        };

        self.expect(RustTokenType::Equal)?;
        let initializer = Some(self.parse_expression()?);

        if *self.current_token() == RustTokenType::Semicolon {
            self.advance()?;
        }

        Ok(Statement::VariableDeclaration { name, var_type, initializer, is_mutable })
    }

    fn parse_return_statement(&mut self) -> Result<Statement> {
        self.expect(RustTokenType::Return)?;

        let value = if *self.current_token() == RustTokenType::Semicolon || *self.current_token() == RustTokenType::Newline {
            None
        }
        else {
            Some(self.parse_expression()?)
        };

        if *self.current_token() == RustTokenType::Semicolon {
            self.advance()?;
        }

        Ok(Statement::Return(value))
    }

    fn parse_expression(&mut self) -> Result<Expression> {
        self.parse_range()
    }

    fn parse_range(&mut self) -> Result<Expression> {
        let mut expr = self.parse_logical_or()?;

        if *self.current_token() == RustTokenType::DotDot {
            self.advance()?;
            let end = self.parse_logical_or()?;
            expr = Expression::Range { start: Box::new(expr), end: Box::new(end) };
        }

        Ok(expr)
    }

    fn parse_logical_or(&mut self) -> Result<Expression> {
        let mut expr = self.parse_logical_and()?;

        while *self.current_token() == RustTokenType::OrOr {
            self.advance()?;
            let right = self.parse_logical_and()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator: BinaryOperator::Or, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expression> {
        let mut expr = self.parse_equality()?;

        while *self.current_token() == RustTokenType::AndAnd {
            self.advance()?;
            let right = self.parse_equality()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator: BinaryOperator::And, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expression> {
        let mut expr = self.parse_comparison()?;

        while matches!(self.current_token(), RustTokenType::EqualEqual | RustTokenType::NotEqual) {
            let operator = match self.current_token() {
                RustTokenType::EqualEqual => BinaryOperator::Equal,
                RustTokenType::NotEqual => BinaryOperator::NotEqual,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_comparison()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expression> {
        let mut expr = self.parse_term()?;

        while matches!(
            self.current_token(),
            RustTokenType::Greater | RustTokenType::GreaterEqual | RustTokenType::Less | RustTokenType::LessEqual
        ) {
            let operator = match self.current_token() {
                RustTokenType::Greater => BinaryOperator::Greater,
                RustTokenType::GreaterEqual => BinaryOperator::GreaterEqual,
                RustTokenType::Less => BinaryOperator::Less,
                RustTokenType::LessEqual => BinaryOperator::LessEqual,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_term()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expression> {
        let mut expr = self.parse_factor()?;

        while matches!(self.current_token(), RustTokenType::Plus | RustTokenType::Minus) {
            let operator = match self.current_token() {
                RustTokenType::Plus => BinaryOperator::Add,
                RustTokenType::Minus => BinaryOperator::Subtract,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_factor()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expression> {
        let mut expr = self.parse_unary()?;

        while matches!(self.current_token(), RustTokenType::Star | RustTokenType::Slash) {
            let operator = match self.current_token() {
                RustTokenType::Star => BinaryOperator::Multiply,
                RustTokenType::Slash => BinaryOperator::Divide,
                _ => unreachable!(),
            };
            self.advance()?;
            let right = self.parse_unary()?;
            expr = Expression::BinaryOperation { left: Box::new(expr), operator, right: Box::new(right) };
        }

        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expression> {
        match self.current_token() {
            RustTokenType::Bang => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expression::UnaryOperation { operator: UnaryOperator::Not, operand: Box::new(operand) })
            }
            RustTokenType::Minus => {
                self.advance()?;
                let operand = self.parse_unary()?;
                Ok(Expression::UnaryOperation { operator: UnaryOperator::Negate, operand: Box::new(operand) })
            }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> Result<Expression> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.current_token() {
                RustTokenType::LeftParen => {
                    self.advance()?;
                    let mut arguments = Vec::new();

                    while *self.current_token() != RustTokenType::RightParen {
                        arguments.push(self.parse_expression()?);

                        if *self.current_token() == RustTokenType::Comma {
                            self.advance()?;
                        }
                        else if *self.current_token() != RustTokenType::RightParen {
                            return Err(GaiaError::syntax_error("期望 ',' 或 ')'".to_string(), SourceLocation::default()));
                        }
                    }

                    self.expect(RustTokenType::RightParen)?;

                    expr = match expr {
                        Expression::Identifier(name) => Expression::FunctionCall { name, arguments },
                        _ => return Err(GaiaError::syntax_error("只能调用标识符".to_string(), SourceLocation::default())),
                    };
                }
                RustTokenType::Dot => {
                    self.advance()?;
                    let member_name = match self.current_token() {
                        RustTokenType::Identifier => {
                            let name = self.current_token_text();
                            self.advance()?;
                            name
                        }
                        _ => return Err(GaiaError::syntax_error("期望成员名".to_string(), SourceLocation::default())),
                    };

                    if *self.current_token() == RustTokenType::LeftParen {
                        // 方法调用
                        self.advance()?;
                        let mut arguments = Vec::new();

                        while *self.current_token() != RustTokenType::RightParen {
                            arguments.push(self.parse_expression()?);

                            if *self.current_token() == RustTokenType::Comma {
                                self.advance()?;
                            }
                            else if *self.current_token() != RustTokenType::RightParen {
                                return Err(GaiaError::syntax_error("期望 ',' 或 ')'".to_string(), SourceLocation::default()));
                            }
                        }

                        self.expect(RustTokenType::RightParen)?;
                        expr = Expression::MethodCall { object: Box::new(expr), method: member_name, arguments };
                    }
                    else {
                        // 字段访问
                        expr = Expression::FieldAccess { object: Box::new(expr), field: member_name };
                    }
                }
                RustTokenType::LeftBracket => {
                    // 索引访问
                    self.advance()?;
                    let index = self.parse_expression()?;
                    self.expect(RustTokenType::RightBracket)?;
                    expr = Expression::IndexAccess { object: Box::new(expr), index: Box::new(index) };
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expression> {
        match self.current_token() {
            RustTokenType::Integer => {
                let text = self.current_token_text();
                let value = text
                    .parse::<i64>()
                    .map_err(|_| GaiaError::syntax_error("无效的整数".to_string(), SourceLocation::default()))?;
                self.advance()?;
                Ok(Expression::Literal(Literal::Integer(value)))
            }
            RustTokenType::Float => {
                let text = self.current_token_text();
                let value = text
                    .parse::<f64>()
                    .map_err(|_| GaiaError::syntax_error("无效的浮点数".to_string(), SourceLocation::default()))?;
                self.advance()?;
                Ok(Expression::Literal(Literal::Float(value)))
            }
            RustTokenType::StringLiteral => {
                let text = self.current_token_text();
                // 移除引号
                let value = if text.len() >= 2 && text.starts_with('"') && text.ends_with('"') {
                    text[1..text.len() - 1].to_string()
                }
                else {
                    text
                };
                self.advance()?;
                Ok(Expression::Literal(Literal::String(value)))
            }
            RustTokenType::CharLiteral => {
                let text = self.current_token_text();
                // 移除单引号
                let value = if text.len() >= 3 && text.starts_with('\'') && text.ends_with('\'') {
                    text[1..text.len() - 1].chars().next().unwrap_or('\0')
                }
                else {
                    '\0'
                };
                self.advance()?;
                Ok(Expression::Literal(Literal::Char(value)))
            }
            RustTokenType::True => {
                self.advance()?;
                Ok(Expression::Literal(Literal::Boolean(true)))
            }
            RustTokenType::False => {
                self.advance()?;
                Ok(Expression::Literal(Literal::Boolean(false)))
            }
            RustTokenType::Identifier => {
                let name = self.current_token_text();
                self.advance()?;

                if *self.current_token() == RustTokenType::LeftBrace {
                    // 结构体实例化
                    self.advance()?;
                    let mut fields = Vec::new();

                    while *self.current_token() != RustTokenType::RightBrace {
                        let field_name = match self.current_token() {
                            RustTokenType::Identifier => {
                                let name = self.current_token_text();
                                self.advance()?;
                                name
                            }
                            _ => return Err(GaiaError::syntax_error("期望字段名".to_string(), SourceLocation::default())),
                        };

                        self.expect(RustTokenType::Colon)?;
                        let value = self.parse_expression()?;
                        fields.push((field_name, value));

                        if *self.current_token() == RustTokenType::Comma {
                            self.advance()?;
                            self.skip_newlines()?;
                        }
                        else if *self.current_token() != RustTokenType::RightBrace {
                            return Err(GaiaError::syntax_error("期望 ',' 或 '}'".to_string(), SourceLocation::default()));
                        }
                    }

                    self.expect(RustTokenType::RightBrace)?;
                    Ok(Expression::StructInstantiation { name, fields })
                }
                else {
                    Ok(Expression::Identifier(name))
                }
            }
            RustTokenType::LeftParen => {
                self.advance()?;
                let expr = self.parse_expression()?;
                self.expect(RustTokenType::RightParen)?;
                Ok(expr)
            }
            RustTokenType::LeftBracket => {
                // 数组字面量: [1, 2, 3]
                self.advance()?;
                let mut elements = Vec::new();
                while *self.current_token() != RustTokenType::RightBracket {
                    elements.push(self.parse_expression()?);
                    if *self.current_token() == RustTokenType::Comma {
                        self.advance()?;
                        self.skip_newlines()?;
                    }
                    else if *self.current_token() != RustTokenType::RightBracket {
                        return Err(GaiaError::syntax_error("期望 ',' 或 ']'".to_string(), SourceLocation::default()));
                    }
                }
                self.expect(RustTokenType::RightBracket)?;
                Ok(Expression::ArrayLiteral(elements))
            }
            _ => Err(GaiaError::syntax_error(format!("意外的 token: {:?}", self.current_token()), SourceLocation::default())),
        }
    }
}
