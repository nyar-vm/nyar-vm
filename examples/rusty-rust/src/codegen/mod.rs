//! Rusty Rust 代码生成器

use crate::ast::*;
use gaia_assembler::{
    instruction::{CmpCondition, CoreInstruction, GaiaInstruction},
    program::{GaiaConstant, GaiaFunction, GaiaModule, GaiaStruct},
    types::GaiaType,
};
use nyar_error::NyarError;
use nyar_types::IKunTree;
use oak_rust::RustRoot;
use std::collections::{HashMap, HashSet};

/// Gaia 转换器
pub struct GaiaTranslator {
    /// 结构体定义映射
    structs: HashMap<String, StructDefinition>,
    /// 参数映射
    parameters: HashMap<String, usize>,
    /// 局部变量映射
    locals: HashMap<String, usize>,
    /// 变量类型映射 (用于字段访问)
    variable_types: HashMap<String, String>,
    /// 可变变量集合（包括参数和局部变量）
    mutable_vars: HashSet<String>,
    /// 当前局部变量索引
    local_index: usize,
    /// 标签计数器
    label_count: usize,
    /// 变量 Gaia 类型映射
    variable_gaia_types: HashMap<String, GaiaType>,
}

impl GaiaTranslator {
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            parameters: HashMap::new(),
            locals: HashMap::new(),
            variable_types: HashMap::new(),
            variable_gaia_types: HashMap::new(),
            mutable_vars: HashSet::new(),
            local_index: 0,
            label_count: 0,
        }
    }

    /// 转换为 IKunTree
    pub fn translate_to_tree(&self, _ast: &RustRoot) -> Result<IKunTree, NyarError> {
        // TODO: 实现真正的从 RustRoot 到 IKunTree 的转换
        Ok(IKunTree::Module("rusty-rust-program".to_string(), Vec::new()))
    }

    /// 推断表达式类型
    fn infer_type(&self, expr: &Expression) -> GaiaType {
        match expr {
            Expression::Literal(literal) => match literal {
                Literal::Integer(_) => GaiaType::I32,
                Literal::Float(_) => GaiaType::F64,
                Literal::Boolean(_) => GaiaType::Bool,
                Literal::String(_) => GaiaType::String,
                Literal::Char(_) => GaiaType::I32, // char 在 CLR 中通常作为 i32 处理或 u16
            },
            Expression::Identifier(name) => {
                if let Some(gaia_type) = self.variable_gaia_types.get(name) {
                    return gaia_type.clone();
                }
                if let Some(type_name) = self.variable_types.get(name) {
                    if let Some(_) = self.structs.get(type_name) {
                        return GaiaType::Object;
                    }
                }
                // 如果没有记录类型，默认返回 i32 (简化处理)
                GaiaType::I32
            }
            Expression::ArrayLiteral(elements) => {
                if let Some(first) = elements.first() {
                    GaiaType::Array(Box::new(self.infer_type(first)), elements.len())
                } else {
                    GaiaType::Array(Box::new(GaiaType::I32), 0)
                }
            }
            Expression::IndexAccess { object, .. } => match self.infer_type(object) {
                GaiaType::Array(elem_type, _) => *elem_type,
                _ => GaiaType::I32,
            },
            Expression::FieldAccess { object, field } => {
                let obj_type = self.infer_type(object);
                let struct_name = match obj_type {
                    GaiaType::Object => {
                        match object.as_ref() {
                            Expression::Identifier(name) => self
                                .variable_types
                                .get(name)
                                .cloned()
                                .unwrap_or_else(|| "Object".to_string()),
                            Expression::FieldAccess { .. } => {
                                // 递归获取嵌套字段的类型名称
                                let field_type = self.infer_type(object);
                                if let GaiaType::Object = field_type {
                                    // 这是一个简化实现，实际上我们需要更复杂的类型跟踪
                                    // 目前假设返回 GaiaType::Object 的字段访问，其对应的结构体名记录在某处
                                    // 或者我们直接返回字段定义的类型
                                    "Object".to_string()
                                } else {
                                    "Object".to_string()
                                }
                            }
                            _ => "Object".to_string(),
                        }
                    }
                    _ => "Object".to_string(),
                };

                // 如果能确定结构体名，查找字段定义以获取准确类型
                if struct_name != "Object" {
                    if let Some(struct_def) = self.structs.get(&struct_name) {
                        if let Some(field_def) = struct_def.fields.iter().find(|f| f.name == *field)
                        {
                            return field_def.field_type.to_gaia_type();
                        }
                    }
                }

                // 备选方案：如果是嵌套字段访问且 object 也是 FieldAccess
                if let Expression::FieldAccess {
                    object: inner_obj,
                    field: inner_field,
                } = object.as_ref()
                {
                    let inner_struct_type = self.infer_type(inner_obj);
                    let inner_struct_name = match inner_struct_type {
                        GaiaType::Object => {
                            if let Expression::Identifier(name) = inner_obj.as_ref() {
                                self.variable_types
                                    .get(name)
                                    .cloned()
                                    .unwrap_or_else(|| "Object".to_string())
                            } else {
                                "Object".to_string()
                            }
                        }
                        _ => "Object".to_string(),
                    };

                    if let Some(inner_struct_def) = self.structs.get(&inner_struct_name) {
                        if let Some(inner_field_def) = inner_struct_def
                            .fields
                            .iter()
                            .find(|f| f.name == *inner_field)
                        {
                            if let Type::Custom(nest_struct_name) = &inner_field_def.field_type {
                                if let Some(nest_struct_def) = self.structs.get(nest_struct_name) {
                                    if let Some(f_def) =
                                        nest_struct_def.fields.iter().find(|f| f.name == *field)
                                    {
                                        return f_def.field_type.to_gaia_type();
                                    }
                                }
                            }
                        }
                    }
                }

                GaiaType::I32
            }
            Expression::BinaryOperation { left, operator, .. } => match operator {
                BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterEqual
                | BinaryOperator::And
                | BinaryOperator::Or => GaiaType::Bool,
                _ => self.infer_type(left),
            },
            Expression::UnaryOperation { operator, operand } => match operator {
                UnaryOperator::Not => GaiaType::Bool,
                UnaryOperator::Negate => self.infer_type(operand),
            },
            Expression::StructInstantiation { .. } => GaiaType::Object,
            Expression::FunctionCall { .. }
            | Expression::MethodCall { .. }
            | Expression::MacroCall { .. } => {
                // 简化处理，默认返回 i32 或 Void
                GaiaType::I32
            }
            Expression::Range { .. } => GaiaType::Object, // Range 作为一个对象处理
        }
    }

    /// 获取结构体名称
    fn get_struct_name(&self, expr: &Expression) -> String {
        match self.infer_type(expr) {
            GaiaType::Object => match expr {
                Expression::Identifier(name) => self
                    .variable_types
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| "Object".to_string()),
                Expression::FieldAccess { object, field: _ } => {
                    let struct_name = self.get_struct_name(object);
                    if let Some(s) = self.structs.get(&struct_name) {
                        for f in &s.fields {
                            if let Type::Custom(name) = &f.field_type {
                                return name.clone();
                            }
                        }
                    }
                    "Object".to_string()
                }
                Expression::StructInstantiation { name, .. } => name.clone(),
                _ => "Object".to_string(),
            },
            _ => "Object".to_string(),
        }
    }

    /// 生成唯一标签
    fn new_label(&mut self, prefix: &str) -> String {
        let label = format!("{}_{}", prefix, self.label_count);
        self.label_count += 1;
        label
    }

    /// 生成 GaiaModule
    pub fn generate(&mut self, program: &Program) -> Result<GaiaModule, NyarError> {
        let mut functions = Vec::new();
        let mut structs = Vec::new();

        // 记录结构体定义
        for struct_def in &program.structs {
            self.structs
                .insert(struct_def.name.clone(), struct_def.clone());

            // 转换为 GaiaStruct
            let mut fields = Vec::new();
            for field in &struct_def.fields {
                fields.push((field.name.clone(), field.field_type.to_gaia_type()));
            }
            structs.push(GaiaStruct {
                name: struct_def.name.clone(),
                fields,
            });
        }

        for function in &program.functions {
            functions.push(self.generate_function(function)?);
        }

        Ok(GaiaModule {
            name: program.name.clone(),
            functions,
            structs,
            constants: Vec::new(),
            globals: Vec::new(),
            classes: Vec::new(),
            imports: Vec::new(),
        })
    }

    /// 生成函数
    fn generate_function(&mut self, function: &Function) -> Result<GaiaFunction, NyarError> {
        // 重置状态
        self.parameters.clear();
        self.locals.clear();
        self.variable_types.clear();
        self.variable_gaia_types.clear();
        self.mutable_vars.clear();
        self.local_index = 0;

        // 处理参数类型和签名
        let mut param_types: Vec<GaiaType> = Vec::new();
        for (i, param) in function.parameters.iter().enumerate() {
            let gaia_type = param.param_type.to_gaia_type();
            param_types.push(gaia_type.clone());

            self.parameters.insert(param.name.clone(), i);
            self.variable_gaia_types
                .insert(param.name.clone(), gaia_type.clone());

            if let Type::Custom(name) = &param.param_type {
                self.variable_types.insert(param.name.clone(), name.clone());
            }
            if param.is_mutable {
                self.mutable_vars.insert(param.name.clone());
            }
        }

        let mut instructions = Vec::new();

        // 生成函数体
        self.generate_block(&function.body, &mut instructions)?;

        // 处理返回值
        let return_type = function
            .return_type
            .as_ref()
            .map(|t| t.to_gaia_type())
            .unwrap_or(GaiaType::Void);

        // 如果函数没有显式返回，添加默认返回
        // 在新版中，Ret 是 CoreInstruction
        if !instructions
            .iter()
            .any(|inst| matches!(inst, GaiaInstruction::Core(CoreInstruction::Ret)))
        {
            // 特殊处理 main 函数：如果推断返回 i32，则压入 0
            if function.name == "main" && return_type == GaiaType::Void {
                // main 默认返回 0
                instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                    GaiaConstant::I32(0),
                )));
            }
            // 注意：terminator 也会处理返回，这里添加 Ret 指令是为了兼容旧逻辑或作为显式返回
        }

        Ok(GaiaFunction {
            name: function.name.clone(),
            signature: GaiaSignature {
                params: param_types,
                return_type,
            },
            blocks: vec![GaiaBlock {
                label: "entry".to_string(),
                instructions,
                terminator: GaiaTerminator::Return,
            }],
            is_external: false,
        })
    }

    /// 生成代码块
    fn generate_block(
        &mut self,
        block: &Block,
        instructions: &mut Vec<GaiaInstruction>,
    ) -> Result<(), NyarError> {
        for statement in &block.statements {
            self.generate_statement(statement, instructions)?;
        }
        Ok(())
    }

    /// 生成语句
    fn generate_statement(
        &mut self,
        statement: &Statement,
        instructions: &mut Vec<GaiaInstruction>,
    ) -> Result<(), NyarError> {
        match statement {
            Statement::Expression(expr) => {
                self.generate_expression(expr, instructions)?;
                // 如果表达式有返回值，弹出它（除非是语句的一部分，这里简单处理）
                let expr_type = self.infer_type(expr);
                if expr_type != GaiaType::Void {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Pop(expr_type)));
                }
            }
            Statement::VariableDeclaration {
                name,
                var_type,
                initializer,
                is_mutable,
            } => {
                let index = self.local_index;
                self.local_index += 1;
                self.locals.insert(name.clone(), index);

                let gaia_type = var_type
                    .as_ref()
                    .map(|t| t.to_gaia_type())
                    .unwrap_or(GaiaType::I32);
                self.variable_gaia_types.insert(name.clone(), gaia_type.clone());

                if let Some(Type::Custom(struct_name)) = var_type {
                    self.variable_types.insert(name.clone(), struct_name.clone());
                }

                if *is_mutable {
                    self.mutable_vars.insert(name.clone());
                }

                if let Some(init) = initializer {
                    self.generate_expression(init, instructions)?;
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(
                        index as u32,
                        gaia_type,
                    )));
                }
            }
            Statement::Return(expr) => {
                if let Some(e) = expr {
                    self.generate_expression(e, instructions)?;
                }
                instructions.push(GaiaInstruction::Core(CoreInstruction::Ret));
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let else_label = self.new_label("else");
                let end_label = self.new_label("end_if");

                self.generate_expression(condition, instructions)?;
                instructions.push(GaiaInstruction::Core(CoreInstruction::BrFalse(
                    else_label.clone(),
                )));

                self.generate_block(then_branch, instructions)?;
                instructions.push(GaiaInstruction::Core(CoreInstruction::Br(end_label.clone())));

                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(else_label)));
                if let Some(else_b) = else_branch {
                    self.generate_block(else_b, instructions)?;
                }

                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(end_label)));
            }
            Statement::While { condition, body } => {
                let start_label = self.new_label("while_start");
                let end_label = self.new_label("while_end");

                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(
                    start_label.clone(),
                )));
                self.generate_expression(condition, instructions)?;
                instructions.push(GaiaInstruction::Core(CoreInstruction::BrFalse(
                    end_label.clone(),
                )));

                self.generate_block(body, instructions)?;
                instructions.push(GaiaInstruction::Core(CoreInstruction::Br(start_label)));

                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(end_label)));
            }
            Statement::For {
                var_name,
                iterable,
                body,
            } => {
                if let Expression::Range { start, end } = iterable {
                    let start_label = self.new_label("for_start");
                    let end_label = self.new_label("for_end");

                    // 1. 初始化变量
                    let index = self.local_index;
                    self.local_index += 1;
                    self.locals.insert(var_name.clone(), index);
                    self.generate_expression(start, instructions)?;
                    let var_type = GaiaType::I32; // 范围迭代目前仅支持 i32
                    self.variable_gaia_types
                        .insert(var_name.clone(), var_type.clone());
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(
                        index as u32,
                        var_type.clone(),
                    )));

                    // 2. 循环开始标签
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Label(
                        start_label.clone(),
                    )));

                    // 3. 检查条件 (i < end)
                    instructions.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(
                        index as u32,
                        var_type.clone(),
                    )));
                    self.generate_expression(end, instructions)?;
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Cmp(
                        CmpCondition::Lt,
                        var_type.clone(),
                    )));
                    instructions.push(GaiaInstruction::Core(CoreInstruction::BrFalse(
                        end_label.clone(),
                    )));

                    // 4. 循环体
                    self.generate_block(body, instructions)?;

                    // 5. 递增变量 (i = i + 1)
                    instructions.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(
                        index as u32,
                        var_type.clone(),
                    )));
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::I32(1),
                    )));
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Add(
                        var_type.clone(),
                    )));
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(
                        index as u32,
                        var_type,
                    )));

                    // 6. 跳转回开始
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Br(start_label)));

                    // 7. 结束标签
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Label(end_label)));
                } else {
                    return Err(NyarError::Lower(
                        "目前 for 循环仅支持范围表达式 (start..end)".to_string(),
                    ));
                }
            }
            Statement::Assignment { target, value } => {
                match target {
                    Expression::Identifier(name) => {
                        // 检查变量是否存在
                        let is_param = self.parameters.contains_key(name);
                        let is_local = self.locals.contains_key(name);

                        if !is_param && !is_local {
                            return Err(NyarError::Lower(format!("未定义的变量: {}", name)));
                        }

                        // 检查可变性
                        if !self.mutable_vars.contains(name) {
                            return Err(NyarError::Lower(format!(
                                "不能对不可变变量赋值: {}",
                                name
                            )));
                        }

                        // 生成值
                        self.generate_expression(value, instructions)?;

                        // 存储
                        let gaia_type = self
                            .variable_gaia_types
                            .get(name)
                            .cloned()
                            .unwrap_or(GaiaType::I32);
                        if let Some(&index) = self.parameters.get(name) {
                            instructions.push(GaiaInstruction::Core(CoreInstruction::StoreArg(
                                index as u32,
                                gaia_type,
                            )));
                        } else if let Some(&index) = self.locals.get(name) {
                            instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(
                                index as u32,
                                gaia_type,
                            )));
                        }
                    }
                    Expression::FieldAccess { object, field } => {
                        // 1. 加载对象
                        self.generate_expression(object, instructions)?;
                        // 2. 加载值
                        self.generate_expression(value, instructions)?;
                        // 3. 存储字段
                        let struct_name = self.get_struct_name(object);
                        instructions.push(GaiaInstruction::Core(CoreInstruction::StoreField(
                            struct_name,
                            field.clone(),
                        )));
                    }
                    Expression::IndexAccess { object, index } => {
                        // 1. 加载数组
                        self.generate_expression(object, instructions)?;
                        // 2. 加载索引
                        self.generate_expression(index, instructions)?;
                        // 3. 加载值
                        self.generate_expression(value, instructions)?;
                        // 4. 存储元素
                        let elem_type = match self.infer_type(object) {
                            GaiaType::Array(t, _) => *t,
                            _ => GaiaType::I32,
                        };
                        instructions.push(GaiaInstruction::Core(CoreInstruction::StoreElement(
                            elem_type,
                        )));
                    }
                    _ => {
                        return Err(NyarError::Lower(format!(
                            "不支持的赋值目标: {:?}",
                            target
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    /// 生成表达式
    fn generate_expression(
        &mut self,
        expression: &Expression,
        instructions: &mut Vec<GaiaInstruction>,
    ) -> Result<(), NyarError> {
        match expression {
            Expression::Literal(literal) => match literal {
                Literal::Integer(i) => {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::I32(*i as i32),
                    )));
                }
                Literal::Float(f) => {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::F64(*f),
                    )));
                }
                Literal::Boolean(b) => {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::Bool(*b),
                    )));
                }
                Literal::String(s) => {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::String(s.clone()),
                    )));
                }
                Literal::Char(c) => {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::I32(*c as i32),
                    )));
                }
            },
            Expression::Identifier(name) => {
                let gaia_type = self
                    .variable_gaia_types
                    .get(name)
                    .cloned()
                    .unwrap_or(GaiaType::I32);
                if let Some(&index) = self.parameters.get(name) {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::LoadArg(
                        index as u32,
                        gaia_type,
                    )));
                } else if let Some(&index) = self.locals.get(name) {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::LoadLocal(
                        index as u32,
                        gaia_type,
                    )));
                } else {
                    return Err(NyarError::Lower(format!("未定义的标识符: {}", name)));
                }
            }
            Expression::BinaryOperation {
                left,
                operator,
                right,
            } => {
                self.generate_expression(left, instructions)?;
                self.generate_expression(right, instructions)?;

                let left_type = self.infer_type(left);
                let instruction = match operator {
                    BinaryOperator::Add => GaiaInstruction::Core(CoreInstruction::Add(left_type)),
                    BinaryOperator::Subtract => {
                        GaiaInstruction::Core(CoreInstruction::Sub(left_type))
                    }
                    BinaryOperator::Multiply => {
                        GaiaInstruction::Core(CoreInstruction::Mul(left_type))
                    }
                    BinaryOperator::Divide => GaiaInstruction::Core(CoreInstruction::Div(left_type)),
                    BinaryOperator::Equal => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Eq, left_type))
                    }
                    BinaryOperator::NotEqual => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Ne, left_type))
                    }
                    BinaryOperator::Less => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Lt, left_type))
                    }
                    BinaryOperator::LessEqual => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Le, left_type))
                    }
                    BinaryOperator::Greater => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Gt, left_type))
                    }
                    BinaryOperator::GreaterEqual => {
                        GaiaInstruction::Core(CoreInstruction::Cmp(CmpCondition::Ge, left_type))
                    }
                    BinaryOperator::And => {
                        GaiaInstruction::Core(CoreInstruction::And(GaiaType::Bool))
                    }
                    BinaryOperator::Or => GaiaInstruction::Core(CoreInstruction::Or(GaiaType::Bool)),
                };

                instructions.push(instruction);
            }
            Expression::UnaryOperation { operator, operand } => {
                let op_type = self.infer_type(operand);
                match operator {
                    UnaryOperator::Negate => {
                        self.generate_expression(operand, instructions)?;
                        instructions.push(GaiaInstruction::Core(CoreInstruction::Neg(op_type)));
                    }
                    UnaryOperator::Not => {
                        self.generate_expression(operand, instructions)?;
                        instructions.push(GaiaInstruction::Core(CoreInstruction::Not(op_type)));
                    }
                }
            }
            Expression::FunctionCall { name, arguments } => {
                for arg in arguments {
                    self.generate_expression(arg, instructions)?;
                }
                instructions.push(GaiaInstruction::Core(CoreInstruction::Call(
                    name.clone(),
                    arguments.len(),
                )));
            }
            Expression::MethodCall {
                object,
                method,
                arguments,
            } => {
                if let Expression::Identifier(obj_name) = object.as_ref() {
                    if obj_name == "console" && method == "log" {
                        for arg in arguments {
                            self.generate_expression(arg, instructions)?;
                        }
                        instructions.push(GaiaInstruction::Core(CoreInstruction::Call(
                            "console.log".to_string(),
                            arguments.len(),
                        )));
                        return Ok(());
                    }
                }

                if method == "len" && arguments.is_empty() {
                    let obj_type = self.infer_type(object);
                    if let GaiaType::Array(_, _) = obj_type {
                        self.generate_expression(object, instructions)?;
                        instructions.push(GaiaInstruction::Core(CoreInstruction::ArrayLength));
                        return Ok(());
                    }
                }

                return Err(NyarError::Lower(format!(
                    "不支持的方法调用: {}.{}",
                    self.expression_to_string(object),
                    method
                )));
            }
            Expression::MacroCall { name, arguments } => {
                if name == "println" {
                    for arg in arguments {
                        self.generate_expression(arg, instructions)?;
                    }
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Call(
                        "println".to_string(),
                        arguments.len(),
                    )));
                } else {
                    return Err(NyarError::Lower(format!("不支持的宏: {}!", name)));
                }
            }
            Expression::StructInstantiation { name, fields } => {
                instructions.push(GaiaInstruction::Core(CoreInstruction::New(name.clone())));
                for field_init in fields {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Dup));
                    self.generate_expression(&field_init.value, instructions)?;
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreField(
                        name.clone(),
                        field_init.name.clone(),
                    )));
                }
            }
            Expression::FieldAccess { object, field } => {
                self.generate_expression(object, instructions)?;
                let struct_name = self.get_struct_name(object);
                instructions.push(GaiaInstruction::Core(CoreInstruction::LoadField(
                    struct_name,
                    field.clone(),
                )));
            }
            Expression::Range { .. } => {
                return Err(NyarError::Lower(
                    "范围表达式仅支持在 for 循环中使用".to_string(),
                ));
            }
            Expression::ArrayLiteral(elements) => {
                let elem_type = if let Some(first) = elements.first() {
                    self.infer_type(first)
                } else {
                    GaiaType::I32
                };

                instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                    GaiaConstant::I32(elements.len() as i32),
                )));
                instructions.push(GaiaInstruction::Core(CoreInstruction::NewArray(
                    elem_type.clone(),
                )));

                for (i, expr) in elements.iter().enumerate() {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Dup));
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::I32(i as i32),
                    )));
                    self.generate_expression(expr, instructions)?;
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreElement(
                        elem_type.clone(),
                    )));
                }
            }
            Expression::IndexAccess { object, index } => {
                self.generate_expression(object, instructions)?;
                self.generate_expression(index, instructions)?;
                let elem_type = self.infer_type(expression);
                instructions.push(GaiaInstruction::Core(CoreInstruction::LoadElement(
                    elem_type,
                )));
            }
        }
        Ok(())
    }

    /// 将表达式转换为字符串（用于错误消息）
    pub fn expression_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(name) => name.clone(),
            Expression::Literal(Literal::String(s)) => format!("\"{}\"", s),
            Expression::Literal(Literal::Integer(i)) => i.to_string(),
            Expression::Literal(Literal::Float(f)) => f.to_string(),
            Expression::Literal(Literal::Boolean(b)) => b.to_string(),
            _ => "<expression>".to_string(),
        }
    }
}
