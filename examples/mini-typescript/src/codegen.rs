//! Gaia 指令生成器
//!
//! 将 UAST (Universal Abstract Syntax Tree) 转换为 Gaia 指令
//! 这里的实现将作为 Chomsky 的一部分或与其紧密集成

use chomsky_uast::UastNode;
use gaia_assembler::{
    instruction::{CmpCondition, CoreInstruction, GaiaInstruction, ManagedInstruction},
    program::{GaiaBlock, GaiaConstant, GaiaFunction, GaiaModule, GaiaTerminator},
    types::{GaiaSignature, GaiaType},
};
use gaia_types::GaiaError;
use std::collections::HashMap;

/// Gaia 翻译器，将 UAST 转换为 Gaia 指令
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
    pub fn generate(&mut self, root: &UastNode) -> Result<GaiaModule, GaiaError> {
        let mut functions = Vec::new();

        // 假设 root 是一个 Module
        if let UastNode::Module { items, .. } = root {
            // 生成主函数（包含所有顶级语句，除了函数定义）
            let main_stmts: Vec<&UastNode> = items.iter().filter(|item| !matches!(item, UastNode::Function { .. })).collect();
            if !main_stmts.is_empty() {
                let main_function = self.generate_main_function(&main_stmts)?;
                functions.push(main_function);
            }

            // 生成其他函数定义
            for item in items {
                if let UastNode::Function { name, params, body, .. } = item {
                    let function = self.generate_function(name, params, body)?;
                    functions.push(function);
                }
            }
        } else {
             // 如果 root 不是 Module，尝试将其作为单个语句/表达式处理
             let main_function = self.generate_main_function(&vec![root])?;
             functions.push(main_function);
        }

        // 添加字符串常量
        let mut constants = Vec::new();
        for (s, c) in &self.string_constants {
            constants.push(c.clone());
        }

        Ok(GaiaModule {
            name: "mini_typescript_module".to_string(),
            functions,
            constants,
            globals: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
        })
    }

    fn generate_main_function(&mut self, statements: &[&UastNode]) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();

        for stmt in statements {
            self.generate_statement(stmt)?;
        }

        // 默认返回 0
        self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(0)));
        self.finish_block(GaiaTerminator::Return);

        Ok(GaiaFunction {
            name: "main".to_string(),
            signature: GaiaSignature {
                params: Vec::new(),
                results: vec![GaiaType::I32],
            },
            locals: self.local_types.clone(),
            blocks: std::mem::take(&mut self.blocks),
        })
    }

    fn generate_function(
        &mut self,
        name: &str,
        parameters: &[UastNode],
        body: &[UastNode],
    ) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();

        // 处理参数
        let mut param_types = Vec::new();
        for (i, param) in parameters.iter().enumerate() {
            if let UastNode::Literal(param_name, _) = param {
                // 假设所有参数都是 I32 (简化处理)
                let param_type = GaiaType::I32;
                param_types.push(param_type.clone());
                
                // 将参数映射到局部变量
                self.locals.insert(param_name.clone(), i as u32);
                self.local_types.push(param_type);
                self.local_index += 1;
            }
        }

        // 生成函数体
        for stmt in body {
            self.generate_statement(stmt)?;
        }

        // 确保最后一个块有终止符
        if self.current_instructions.is_empty() || !matches!(self.blocks.last().map(|b| &b.terminator), Some(GaiaTerminator::Return)) {
             self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(0)));
             self.finish_block(GaiaTerminator::Return);
        }

        Ok(GaiaFunction {
            name: name.to_string(),
            signature: GaiaSignature {
                params: param_types,
                results: vec![GaiaType::I32], // 假设返回 I32
            },
            locals: self.local_types.clone(),
            blocks: std::mem::take(&mut self.blocks),
        })
    }

    fn generate_statement(&mut self, statement: &UastNode) -> Result<(), GaiaError> {
        match statement {
            UastNode::Assign { name, value, .. } => {
                // 计算值
                self.generate_expression(value)?;
                
                // 获取或创建局部变量
                let index = if let Some(&idx) = self.locals.get(name) {
                    idx
                } else {
                    let idx = self.local_index;
                    self.locals.insert(name.clone(), idx);
                    self.local_types.push(GaiaType::I32); // 默认 I32
                    self.local_index += 1;
                    idx
                };

                // 存储到局部变量
                self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::LocalSet(index)));
            }
            // 简单的表达式语句
            _ => {
                self.generate_expression(statement)?;
                // 如果表达式产生结果但未被使用，可能需要 Drop (但在 Gaia 中如果栈平衡则不需要显式 Drop)
                // 这里假设 generate_expression 会将结果留在栈顶
                // 对于语句级表达式，我们通常应该 pop 掉结果，除非是特定指令
                self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::Drop));
            }
        }
        Ok(())
    }

    fn generate_expression(&mut self, expression: &UastNode) -> Result<(), GaiaError> {
        match expression {
            UastNode::Literal(val, _) => {
                if let Ok(num) = val.parse::<i32>() {
                    self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(num)));
                } else if val.starts_with('"') || val.starts_with('\'') {
                    // String literal (simplified)
                     let content = val.trim_matches('"').trim_matches('\'').to_string();
                     // Add to constants pool (simplified, just placeholder index)
                     self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(0))); 
                } else {
                    // Identifier (Variable load)
                    if let Some(&index) = self.locals.get(val) {
                         self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::LocalGet(index)));
                    } else {
                        // Undefined variable, push 0 or error (simplified)
                         self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(0)));
                    }
                }
            }
            UastNode::Call { callee, args, .. } => {
                // Evaluate arguments
                for arg in args {
                    self.generate_expression(arg)?;
                }

                match callee.as_str() {
                    "add" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::AddI32)),
                    "sub" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::SubI32)),
                    "mul" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::MulI32)),
                    "div" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::DivI32S)),
                    "eq" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::CmpI32(CmpCondition::Eq))),
                    "lt" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::CmpI32(CmpCondition::Slt))),
                    "gt" => self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::CmpI32(CmpCondition::Sgt))),
                    _ => {
                        // Function call (simplified)
                        // In a real implementation, we would need to resolve the function index/name properly
                         self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::Call(0))); 
                    }
                }
            }
            _ => {
                 // Placeholder for other expressions
                 self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::ConstI32(0)));
            }
        }
        Ok(())
    }
}
