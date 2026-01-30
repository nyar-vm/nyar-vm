//! Mini Rust 代码生成器

use crate::ast::*;
use gaia_assembler::{
    instruction::{CmpCondition, CoreInstruction, GaiaInstruction},
    program::{GaiaBlock, GaiaConstant, GaiaFunction, GaiaModule, GaiaStruct, GaiaTerminator},
    types::{GaiaSignature, GaiaType},
};
use gaia_types::{GaiaError, Result, SourceLocation};
use std::collections::{HashMap, HashSet};

/// 代码生成器
pub struct CodeGenerator {
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

impl CodeGenerator {
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

    /// 获取表达式对应的结构体名称
    fn get_struct_name(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(name) => self
                .variable_types
                .get(name)
                .cloned()
                .unwrap_or_else(|| "Object".to_string()),
            Expression::FieldAccess { object, field } => {
                let obj_struct_name = self.get_struct_name(object);
                if let Some(struct_def) = self.structs.get(&obj_struct_name) {
                    if let Some(field_def) = struct_def.fields.iter().find(|f| f.name == *field) {
                        if let Type::Custom(name) = &field_def.field_type {
                            return name.clone();
                        }
                    }
                }
                "Object".to_string()
            }
            Expression::StructInstantiation { name, .. } => name.clone(),
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
    pub fn generate(&mut self, program: &Program) -> Result<GaiaModule> {
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
    fn generate_function(&mut self, function: &Function) -> Result<GaiaFunction> {
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
    ) -> Result<()> {
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
    ) -> Result<()> {
        match statement {
            Statement::Expression(expr) => {
                self.generate_expression(expr, instructions)?;
                // 表达式语句需要弹出结果，除非是已知的 void 函数调用
                let is_void = match expr {
                    Expression::FunctionCall { name, .. } => name == "print" || name == "println",
                    Expression::MacroCall { name, .. } => name == "println",
                    Expression::MethodCall { object, method, .. } => {
                        if let Expression::Identifier(obj_name) = object.as_ref() {
                            obj_name == "console" && method == "log"
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                if !is_void {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Pop));
                }
            }
            Statement::VariableDeclaration {
                name,
                var_type,
                initializer,
                is_mutable,
            } => {
                let mut gaia_type = GaiaType::I32; // 默认

                if let Some(init_expr) = initializer {
                    // 生成初始值
                    self.generate_expression(init_expr, instructions)?;

                    // 推断类型
                    gaia_type = self.infer_type(init_expr);

                    // 如果初始值是结构体实例化，记录类型名称（用于字段访问）
                    if let Expression::StructInstantiation {
                        name: struct_name, ..
                    } = init_expr
                    {
                        self.variable_types
                            .insert(name.clone(), struct_name.clone());
                    }
                } else {
                    // 如果没有初始值，使用默认值 0
                    instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                        GaiaConstant::I32(0),
                    )));
                }

                // 如果显式指定了类型，覆盖推断的类型
                if let Some(vt) = var_type {
                    gaia_type = vt.to_gaia_type();
                    if let Type::Custom(type_name) = vt {
                        self.variable_types.insert(name.clone(), type_name.clone());
                    }
                }

                // 记录变量的 Gaia 类型
                self.variable_gaia_types
                    .insert(name.clone(), gaia_type.clone());

                // 分配局部变量
                let index = self.local_index;
                self.locals.insert(name.clone(), index);
                if *is_mutable {
                    self.mutable_vars.insert(name.clone());
                }
                self.local_index += 1;

                // 存储到局部变量
                instructions.push(GaiaInstruction::Core(CoreInstruction::StoreLocal(
                    index as u32,
                    gaia_type,
                )));
            }
            Statement::Return(expr) => {
                if let Some(expr) = expr {
                    self.generate_expression(expr, instructions)?;
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

                // 生成条件
                self.generate_expression(condition, instructions)?;
                // 如果为假，跳转到 else 或 end
                instructions.push(GaiaInstruction::Core(CoreInstruction::BrFalse(
                    if else_branch.is_some() {
                        else_label.clone()
                    } else {
                        end_label.clone()
                    },
                )));

                // 生成 then 分支
                self.generate_block(then_branch, instructions)?;

                if let Some(else_branch) = else_branch {
                    // 跳转到结束
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Br(
                        end_label.clone(),
                    )));
                    // 生成 else 标签
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Label(else_label)));
                    // 生成 else 分支
                    self.generate_block(else_branch, instructions)?;
                }

                // 生成结束标签
                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(end_label)));
            }
            Statement::While { condition, body } => {
                let start_label = self.new_label("while_start");
                let end_label = self.new_label("while_end");

                // 生成开始标签
                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(
                    start_label.clone(),
                )));
                // 生成条件
                self.generate_expression(condition, instructions)?;
                // 如果为假，跳转到结束
                instructions.push(GaiaInstruction::Core(CoreInstruction::BrFalse(
                    end_label.clone(),
                )));

                // 生成循环体
                self.generate_block(body, instructions)?;
                // 跳转到开始
                instructions.push(GaiaInstruction::Core(CoreInstruction::Br(start_label)));

                // 生成结束标签
                instructions.push(GaiaInstruction::Core(CoreInstruction::Label(end_label)));
            }
            Statement::For {
                var_name,
                iterable,
                body,
            } => {
                // 目前仅支持 for i in start..end 形式
                if let Expression::Range { start, end } = iterable {
                    let start_label = self.new_label("for_start");
                    let end_label = self.new_label("for_end");

                    // 1. 初始化循环变量
                    self.generate_expression(start, instructions)?;
                    let index = self.local_index;
                    self.locals.insert(var_name.clone(), index);
                    self.mutable_vars.insert(var_name.clone());
                    self.local_index += 1;

                    let var_type = GaiaType::I32; // 范围变量默认为 I32
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
                    return Err(GaiaError::syntax_error(
                        "目前 for 循环仅支持范围表达式 (start..end)".to_string(),
                        SourceLocation::default(),
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
                            return Err(GaiaError::syntax_error(
                                format!("未定义的变量: {}", name),
                                SourceLocation::default(),
                            ));
                        }

                        // 检查可变性
                        if !self.mutable_vars.contains(name) {
                            return Err(GaiaError::syntax_error(
                                format!("不能对不可变变量赋值: {}", name),
                                SourceLocation::default(),
                            ));
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
                        // 2. 生成值
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
                        // 3. 加载新值
                        self.generate_expression(value, instructions)?;

                        // 4. 存储元素
                        let elem_type = self.infer_type(target);
                        instructions.push(GaiaInstruction::Core(CoreInstruction::StoreElement(
                            elem_type,
                        )));
                    }
                    _ => {
                        return Err(GaiaError::syntax_error(
                            "无效的赋值目标".to_string(),
                            SourceLocation::default(),
                        ));
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
    ) -> Result<()> {
        match expression {
            Expression::Literal(literal) => {
                let constant = literal.to_gaia_constant();
                instructions.push(GaiaInstruction::Core(CoreInstruction::PushConstant(
                    constant,
                )));
            }
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
                    return Err(GaiaError::syntax_error(
                        format!("未定义的变量: {}", name),
                        SourceLocation::default(),
                    ));
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
                    BinaryOperator::Divide => {
                        GaiaInstruction::Core(CoreInstruction::Div(left_type))
                    }
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
                    BinaryOperator::Or => {
                        GaiaInstruction::Core(CoreInstruction::Or(GaiaType::Bool))
                    }
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

                return Err(GaiaError::syntax_error(
                    format!(
                        "不支持的方法调用: {}.{}",
                        self.expression_to_string(object),
                        method
                    ),
                    SourceLocation::default(),
                ));
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
                    return Err(GaiaError::syntax_error(
                        format!("不支持的宏: {}!", name),
                        SourceLocation::default(),
                    ));
                }
            }
            Expression::StructInstantiation { name, fields } => {
                instructions.push(GaiaInstruction::Core(CoreInstruction::New(name.clone())));

                for (field_name, value_expr) in fields {
                    instructions.push(GaiaInstruction::Core(CoreInstruction::Dup));
                    self.generate_expression(value_expr, instructions)?;
                    instructions.push(GaiaInstruction::Core(CoreInstruction::StoreField(
                        name.clone(),
                        field_name.clone(),
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
                return Err(GaiaError::syntax_error(
                    "范围表达式仅支持在 for 循环中使用".to_string(),
                    SourceLocation::default(),
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
    fn expression_to_string(&self, expr: &Expression) -> String {
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

/// Mini Rust 解析器
pub struct MiniRustParser;

impl MiniRustParser {
    pub fn parse(source: &str) -> Result<GaiaModule> {
        // 使用 Oak Rust 解析器
        use oak_core::Parser;
        use oak_core::builder::Builder;
        use oak_core::source::SourceText;
        use oak_rust::{RustBuilder, RustLanguage};

        let language = RustLanguage::default();
        let builder = RustBuilder::new(&language);
        let source_text = SourceText::new(source);
        let mut cache = oak_core::parser::ParseSession::default();
        let build_output = builder.build(&source_text, &[], &mut cache);

        if let Err(e) = build_output.result {
            return Err(GaiaError::syntax_error(
                format!("Parse error: {:?}", e),
                SourceLocation::default(),
            ));
        }

        let root = build_output.result.unwrap();

        // 转换为 UIR (如果需要进行优化)
        // let mut egraph = chomsky_uir::EGraph::new();
        // let mut builder = chomsky_uir::IntentBuilder::new(&mut egraph);
        // let root_id = crate::converter::convert_root(&root, &mut builder);
        // TODO: 这里可以插入 Chomsky 优化器 (UIR Optimization)
        // let optimized_ast = crate::converter::from_uir::uir_to_program(&egraph, root_id);

        let mut codegen = CodeGenerator::new();
        codegen.generate(&root)
    }
}
