//! Gaia 指令生成器
//!
//! 将 Python AST 转换为 Gaia 指令

use oak_python::ast::*;
use gaia_assembler::{
    instruction::{CmpCondition, CoreInstruction, GaiaInstruction, ManagedInstruction},
    program::{GaiaBlock, GaiaConstant, GaiaFunction, GaiaModule, GaiaTerminator},
    types::{GaiaSignature, GaiaType},
};
use gaia_types::{GaiaError, SourceLocation};
use std::collections::HashMap;

/// Gaia 翻译器，将 Python AST 转换为 Gaia 指令
pub struct GaiaTranslator {
    /// 局部变量映射
    locals: HashMap<String, u32>,
    /// 局部变量索引计数器
    local_index: u32,
    /// 局部变量类型列表
    local_types: Vec<GaiaType>,
    /// 字符串常量池
    string_constants: Vec<(String, GaiaConstant)>,
    /// 标签计数器
    label_counter: u32,
    /// 当前正在生成的块的指令
    current_instructions: Vec<GaiaInstruction>,
    /// 当前正在生成的块的标签
    current_label: String,
    /// 已完成的块
    blocks: Vec<GaiaBlock>,
}

impl GaiaTranslator {
    /// 创建新的 Gaia 翻译器
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            local_index: 0,
            local_types: Vec::new(),
            string_constants: Vec::new(),
            label_counter: 0,
            current_instructions: Vec::new(),
            current_label: "entry".to_string(),
            blocks: Vec::new(),
        }
    }

    /// 开始一个新块
    fn start_block(&mut self, label: String) {
        self.current_label = label;
        self.current_instructions = Vec::new();
    }

    /// 结束当前块并添加到列表中
    fn finish_block(&mut self, terminator: GaiaTerminator) {
        let block = GaiaBlock {
            label: self.current_label.clone(),
            instructions: std::mem::take(&mut self.current_instructions),
            terminator,
        };
        self.blocks.push(block);
    }

    /// 生成一个新的唯一标签
    fn new_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.label_counter);
        self.label_counter += 1;
        label
    }

    /// 重置翻译器状态（用于生成新函数）
    fn reset_for_function(&mut self) {
        self.locals.clear();
        self.local_index = 0;
        self.local_types.clear();
        self.blocks.clear();
        self.label_counter = 0;
        self.start_block("entry".to_string());
    }

    /// 生成 GaiaModule
    pub fn generate(&mut self, program: &Program) -> Result<GaiaModule, GaiaError> {
        let mut functions = Vec::new();

        // 生成主函数（包含所有顶级语句）
        if !program.statements.is_empty() {
            let main_function = self.generate_main_function(&program.statements)?;
            functions.push(main_function);
        }

        // 生成其他函数定义
        for statement in &program.statements {
            if let Statement::FunctionDef { name, parameters, body, .. } = statement {
                let function = self.generate_function(name, parameters, body)?;
                functions.push(function);
            }
        }

        // 添加字符串常量
        let constants = self.string_constants.clone();

        Ok(GaiaModule {
            name: "mini_python_program".to_string(),
            functions,
            structs: Vec::new(),
            classes: Vec::new(),
            constants,
            globals: Vec::new(),
            imports: Vec::new(),
        })
    }

    /// 生成主函数
    fn generate_main_function(&mut self, statements: &[Statement]) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();

        // 生成所有非函数定义的语句
        for statement in statements {
            if !matches!(statement, Statement::FunctionDef { .. }) {
                self.generate_statement(statement)?;
            }
        }

        self.finish_block(GaiaTerminator::Return);

        Ok(GaiaFunction {
            name: "main".to_string(),
            signature: GaiaSignature { params: Vec::new(), return_type: GaiaType::Void },
            blocks: std::mem::take(&mut self.blocks),
            is_external: false,
        })
    }

    /// 生成函数
    fn generate_function(
        &mut self,
        name: &str,
        parameters: &[Parameter],
        body: &[Statement],
    ) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();

        // 注册参数为局部变量
        for (i, param) in parameters.iter().enumerate() {
            self.locals.insert(param.name.clone(), i as u32);
            self.local_types.push(GaiaType::Object);
            self.local_index += 1;
        }

        for statement in body {
            self.generate_statement(statement)?;
        }

        self.finish_block(GaiaTerminator::Return);

        Ok(GaiaFunction {
            name: name.to_string(),
            signature: GaiaSignature {
                params: vec![GaiaType::Object; parameters.len()],
                return_type: GaiaType::Object,
            },
            blocks: std::mem::take(&mut self.blocks),
            is_external: false,
        })
    }

    /// 生成语句
    fn generate_statement(
        &mut self,
        statement: &Statement,
    ) -> Result<(), GaiaError> {
        match statement {
            Statement::Assignment { target, value } => {
                self.generate_assignment(target, value)?;
            }
            Statement::Expression(expression) => {
                self.generate_expression(expression)?;
                // 表达式语句的结果通常被丢弃，但在 Gaia 中我们可能需要 pop 或者保持栈平衡
                // 这里暂时简单处理
            }
            Statement::Return(expression) => {
                if let Some(expr) = expression {
                    self.generate_expression(expr)?;
                } else {
                    self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(GaiaConstant::Null)));
                }
                self.finish_block(GaiaTerminator::Return);
                // 之后的所有指令都会被丢弃，直到下一个块开始（但这在 linear 生成中不常见）
                let label = self.new_label("unreachable");
                self.start_block(label);
            }
            Statement::If { test, body, orelse } => {
                self.generate_if_statement(test, body, orelse)?;
            }
            Statement::While { test, body, .. } => {
                self.generate_while_statement(test, body)?;
            }
            Statement::Pass => {}
            _ => {
                return Err(GaiaError::syntax_error(
                    format!("Unsupported statement: {:?}", statement),
                    gaia_types::SourceLocation::default(),
                ));
            }
        }
        Ok(())
    }

    /// 生成赋值语句
    fn generate_assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
    ) -> Result<(), GaiaError> {
        // 生成右侧表达式
        self.generate_expression(value)?;

        // 处理左侧目标
        if let Expression::Name(name) = target {
            // 获取或创建局部变量索引
            let local_index = if let Some(&index) = self.locals.get(name) {
                index
            } else {
                let index = self.local_index;
                self.locals.insert(name.clone(), index);
                self.local_types.push(GaiaType::Object);
                self.local_index += 1;
                index
            };

            self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(local_index, GaiaType::Object)));
        } else {
            return Err(GaiaError::syntax_error(
                "Only simple variable assignment is supported".to_string(),
                gaia_types::SourceLocation::default(),
            ));
        }

        Ok(())
    }

    /// 生成表达式
    fn generate_expression(
        &mut self,
        expression: &Expression,
    ) -> Result<(), GaiaError> {
        match expression {
            Expression::Literal(value) => {
                self.generate_literal(value)?;
            }
            Expression::Name(name) => {
                self.generate_variable_load(name)?;
            }
            Expression::BinaryOp { left, operator, right } => {
                self.generate_binary_op(left, operator, right)?;
            }
            Expression::Compare { left, ops, comparators } => {
                self.generate_compare(left, ops, comparators)?;
            }
            Expression::Call { func, args, .. } => {
                self.generate_call(func, args)?;
            }
            _ => {
                return Err(GaiaError::syntax_error(
                    format!("Unsupported expression: {:?}", expression),
                    gaia_types::SourceLocation::default(),
                ));
            }
        }
        Ok(())
    }

    /// 生成字面量
    fn generate_literal(
        &mut self,
        literal: &Literal,
    ) -> Result<(), GaiaError> {
        let constant = match literal {
            Literal::Integer(n) => GaiaConstant::I64(*n),
            Literal::Float(f) => GaiaConstant::F64(*f),
            Literal::String(s) => {
                let const_name = format!("str_{}", self.string_constants.len());
                let constant = GaiaConstant::String(s.clone());
                self.string_constants.push((const_name.clone(), constant.clone()));
                constant
            }
            Literal::Boolean(b) => GaiaConstant::Bool(*b),
            Literal::None => GaiaConstant::Null,
        };

        self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(constant)));
        Ok(())
    }

    /// 生成变量加载
    fn generate_variable_load(
        &mut self,
        name: &str,
    ) -> Result<(), GaiaError> {
        if let Some(&local_index) = self.locals.get(name) {
            self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(local_index, GaiaType::Object)));
        } else {
            return Err(GaiaError::syntax_error(
                format!("Undefined variable: {}", name),
                gaia_types::SourceLocation::default(),
            ));
        }
        Ok(())
    }

    /// 生成比较运算
    fn generate_compare(
        &mut self,
        left: &Expression,
        ops: &[CompareOperator],
        comparators: &[Expression],
    ) -> Result<(), GaiaError> {
        // 简化处理：只支持一个比较运算符
        if ops.len() != 1 || comparators.len() != 1 {
            return Err(GaiaError::syntax_error(
                "Only single comparison operator is supported".to_string(),
                SourceLocation::default(),
            ));
        }

        self.generate_expression(left)?;
        self.generate_expression(&comparators[0])?;

        let cond = match ops[0] {
            CompareOperator::Eq => CmpCondition::Eq,
            CompareOperator::NotEq => CmpCondition::Ne,
            CompareOperator::Lt => CmpCondition::Lt,
            CompareOperator::LtE => CmpCondition::Le,
            CompareOperator::Gt => CmpCondition::Gt,
            CompareOperator::GtE => CmpCondition::Ge,
            _ => {
                return Err(GaiaError::syntax_error(
                    format!("Unsupported comparison operator: {:?}", ops[0]),
                    SourceLocation::default(),
                ));
            }
        };

        self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::Cmp(cond, GaiaType::Object)));
        Ok(())
    }

    /// 生成二元运算
    fn generate_binary_op(
        &mut self,
        left: &Expression,
        operator: &BinaryOperator,
        right: &Expression,
    ) -> Result<(), GaiaError> {
        // 生成左操作数
        self.generate_expression(left)?;
        // 生成右操作数
        self.generate_expression(right)?;

        // 生成运算指令
        let instruction = match operator {
            BinaryOperator::Add => GaiaInstruction::Core(CoreInstruction::Add(GaiaType::Object)),
            BinaryOperator::Sub => GaiaInstruction::Core(CoreInstruction::Sub(GaiaType::Object)),
            BinaryOperator::Mult => GaiaInstruction::Core(CoreInstruction::Mul(GaiaType::Object)),
            BinaryOperator::Div => GaiaInstruction::Core(CoreInstruction::Div(GaiaType::Object)),
            BinaryOperator::FloorDiv => GaiaInstruction::Core(CoreInstruction::Div(GaiaType::Object)), // 简化处理
            BinaryOperator::Mod => GaiaInstruction::Core(CoreInstruction::Rem(GaiaType::Object)),
            BinaryOperator::BitOr => GaiaInstruction::Core(CoreInstruction::Or(GaiaType::Object)),
            BinaryOperator::BitXor => GaiaInstruction::Core(CoreInstruction::Xor(GaiaType::Object)),
            BinaryOperator::BitAnd => GaiaInstruction::Core(CoreInstruction::And(GaiaType::Object)),
            BinaryOperator::LShift => GaiaInstruction::Core(CoreInstruction::Shl(GaiaType::Object)),
            BinaryOperator::RShift => GaiaInstruction::Core(CoreInstruction::Shr(GaiaType::Object)),
            _ => {
                return Err(GaiaError::syntax_error(
                    format!("Unsupported binary operator: {:?}", operator),
                    gaia_types::SourceLocation::default(),
                ));
            }
        };

        self.current_instructions.push(instruction);
        Ok(())
    }

    /// 生成函数调用
    fn generate_call(
        &mut self,
        func: &Expression,
        args: &[Expression],
    ) -> Result<(), GaiaError> {
        // 生成所有参数
        for arg in args {
            self.generate_expression(arg)?;
        }

        // 处理函数调用
        if let Expression::Name(name) = func {
            // 在新的 Gaia IR 中，Call 应该是 terminator 或者是 Managed 指令
            // 这里我们暂时使用 CallStatic 简化处理
            self.current_instructions.push(GaiaInstruction::Managed(ManagedInstruction::CallStatic {
                target: "global".to_string(),
                method: name.clone(),
                signature: GaiaSignature {
                    params: vec![GaiaType::Object; args.len()],
                    return_type: GaiaType::Object,
                },
            }));
        } else {
            return Err(GaiaError::syntax_error(
                "Only direct function calls by name are supported".to_string(),
                SourceLocation::default(),
            ));
        }

        Ok(())
    }

    /// 生成 if 语句
    fn generate_if_statement(
        &mut self,
        test: &Expression,
        body: &[Statement],
        orelse: &[Statement],
    ) -> Result<(), GaiaError> {
        let true_label = self.new_label("if_true");
        let false_label = self.new_label("if_false");
        let end_label = self.new_label("if_end");

        // 1. 计算测试表达式
        self.generate_expression(test)?;

        // 2. 分支跳转
        let has_else = !orelse.is_empty();
        self.finish_block(GaiaTerminator::Branch {
            true_label: true_label.clone(),
            false_label: if has_else { false_label.clone() } else { end_label.clone() },
        });

        // 3. True 块
        self.start_block(true_label);
        for stmt in body {
            self.generate_statement(stmt)?;
        }
        self.finish_block(GaiaTerminator::Jump(end_label.clone()));

        // 4. False 块 (如果有)
        if has_else {
            self.start_block(false_label);
            for stmt in orelse {
                self.generate_statement(stmt)?;
            }
            self.finish_block(GaiaTerminator::Jump(end_label.clone()));
        }

        // 5. 结束块
        self.start_block(end_label);
        Ok(())
    }

    /// 生成 while 语句
    fn generate_while_statement(
        &mut self,
        test: &Expression,
        body: &[Statement],
    ) -> Result<(), GaiaError> {
        let test_label = self.new_label("while_test");
        let body_label = self.new_label("while_body");
        let end_label = self.new_label("while_end");

        // 1. 跳转到测试块
        self.finish_block(GaiaTerminator::Jump(test_label.clone()));

        // 2. 测试块
        self.start_block(test_label.clone());
        self.generate_expression(test)?;
        self.finish_block(GaiaTerminator::Branch {
            true_label: body_label.clone(),
            false_label: end_label.clone(),
        });

        // 3. 循环体块
        self.start_block(body_label);
        for stmt in body {
            self.generate_statement(stmt)?;
        }
        self.finish_block(GaiaTerminator::Jump(test_label));

        // 4. 结束块
        self.start_block(end_label);
        Ok(())
    }
}

impl Default for GaiaTranslator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_assignment() {
        let mut translator = GaiaTranslator::new();

        // 创建简单的赋值语句 AST
        let program = Program {
            statements: vec![Statement::Assignment {
                target: Expression::Name("x".to_string()),
                value: Expression::Literal(Literal::Integer(42)),
            }],
        };

        let result = translator.generate(&program);
        assert!(result.is_ok());

        let gaia_program = result.unwrap();
        assert_eq!(gaia_program.name, "mini_python_program");
        assert!(!gaia_program.functions.is_empty());
    }

    #[test]
    fn test_arithmetic() {
        let mut translator = GaiaTranslator::new();

        // 创建算术表达式 AST: x = 10 + 20
        let program = Program {
            statements: vec![Statement::Assignment {
                target: Expression::Name("x".to_string()),
                value: Expression::BinaryOp {
                    left: Box::new(Expression::Literal(Literal::Integer(10))),
                    operator: BinaryOperator::Add,
                    right: Box::new(Expression::Literal(Literal::Integer(20))),
                },
            }],
        };

        let result = translator.generate(&program);
        assert!(result.is_ok());
    }
}
