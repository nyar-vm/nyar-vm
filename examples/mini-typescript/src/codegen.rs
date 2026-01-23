//! Nyar 指令生成器
//!
//! 将 UAST (Universal Abstract Syntax Tree) 转换为 Nyar 字节码
//! 这里的实现将作为 Chomsky 的一部分或与其紧密集成

use std::collections::HashMap;

use chomsky_uast::UastNode;
use nyar_vm::bytecode::format::{Chunk, Constant, NyarModule};
use nyar_vm::bytecode::opcode::{I32Ext, Opcode};
use nyar_error::FormatError;

/// Nyar 翻译器，将 UAST 转换为 Nyar 字节码
pub struct NyarTranslator {
    /// 局部变量映射
    locals: HashMap<String, u8>,
    /// 局部变量索引计数器
    local_index: u8,
    /// 常量池
    constants: Vec<Constant>,
    /// 标签计数器
    label_counter: u32,
    /// 当前正在生成的代码
    code: Vec<u8>,
    /// 标签位置映射 (标签名 -> 代码偏移)
    labels: HashMap<String, usize>,
    /// 重定位信息 (待修补的偏移 -> 目标标签名, 跳转指令大小)
    relocations: Vec<(usize, String, bool)>, // offset, label, is_conditional
    /// 已完成的 Chunk
    chunks: Vec<Chunk>,
}

impl NyarTranslator {
    /// 创建新的 Nyar 翻译器
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            local_index: 0,
            constants: Vec::new(),
            label_counter: 0,
            code: Vec::new(),
            labels: HashMap::new(),
            relocations: Vec::new(),
            chunks: Vec::new(),
        }
    }

    /// 发射单个字节
    fn emit_u8(&mut self, b: u8) {
        self.code.push(b);
    }

    /// 发射 u16 (小端序)
    fn emit_u16(&mut self, v: u16) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    /// 发射 i16 (小端序)
    fn emit_i16(&mut self, v: i16) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    /// 发射 i32 (小端序)
    fn emit_i32(&mut self, v: i32) {
        self.code.extend_from_slice(&v.to_le_bytes());
    }

    /// 定义标签
    fn define_label(&mut self, name: &str) {
        self.labels.insert(name.to_string(), self.code.len());
    }

    /// 记录重定位
    fn emit_jump(&mut self, label: &str, is_conditional: bool) {
        let op = if is_conditional { Opcode::JumpIfFalse } else { Opcode::Jump };
        self.emit_u8(op as u8);
        let offset = self.code.len();
        self.emit_i16(0); // 占位符
        self.relocations.push((offset, label.to_string(), is_conditional));
    }

    /// 修补所有跳转偏移
    fn patch_jumps(&mut self) {
        for (offset, label, _) in std::mem::take(&mut self.relocations) {
            if let Some(&target) = self.labels.get(&label) {
                let jump_offset = (target as isize - (offset + 2) as isize) as i16;
                let bytes = jump_offset.to_le_bytes();
                self.code[offset] = bytes[0];
                self.code[offset + 1] = bytes[1];
            }
        }
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
        self.code.clear();
        self.labels.clear();
        self.relocations.clear();
        self.label_counter = 0;
    }

    /// 生成 NyarModule
    pub fn generate(&mut self, root: &UastNode) -> Result<NyarModule, FormatError> {
        let mut functions_info = Vec::new();

        // 假设 root 是一个 Module
        if let UastNode::Module { items, .. } = root {
            // 1. 先收集所有函数定义，以便处理调用
            // (这里简化处理，假设所有函数都在 Chunk 中按顺序排列)

            // 2. 生成主 Chunk (包含顶级语句)
            let main_stmts: Vec<&UastNode> = items.iter().filter(|item| !matches!(item, UastNode::Function { .. })).collect();
            self.generate_main_chunk(&main_stmts)?;

            // 3. 生成其他函数的 Chunk
            for item in items {
                if let UastNode::Function { name, params, body, .. } = item {
                    functions_info.push((name.clone(), params.len()));
                    self.generate_function_chunk(name, params, body)?;
                }
            }
        } else {
             self.generate_main_chunk(&[root])?;
        }

        Ok(NyarModule {
            version: 1,
            flags: 0,
            timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs(),
            constants: std::mem::take(&mut self.constants),
            effects: Vec::new(),
            chunks: std::mem::take(&mut self.chunks),
            classes: Vec::new(),
            traits: Vec::new(),
            impls: Vec::new(),
        })
    }

    fn generate_main_chunk(&mut self, statements: &[&UastNode]) -> Result<(), FormatError> {
        self.reset_for_function();

        for stmt in statements {
            self.generate_statement(stmt)?;
        }

        // 默认返回 0
        self.emit_u8(Opcode::I32Ext as u8);
        self.emit_u8(I32Ext::Const as u8);
        self.emit_i32(0);
        self.emit_u8(Opcode::Return as u8);

        self.patch_jumps();
        
        self.chunks.push(Chunk {
            locals: self.local_index as u16,
            upvalues: 0,
            max_stack: 16, // 简化处理
            code: std::mem::take(&mut self.code),
            handlers: Vec::new(),
            lines: Vec::new(),
        });

        Ok(())
    }

    fn generate_function_chunk(
        &mut self,
        _name: &str,
        parameters: &[UastNode],
        body: &[UastNode],
    ) -> Result<(), FormatError> {
        self.reset_for_function();

        // 处理参数
        for (i, param) in parameters.iter().enumerate() {
            if let UastNode::Literal(param_name, _) = param {
                self.locals.insert(param_name.clone(), i as u8);
                self.local_index += 1;
            }
        }

        // 生成函数体
        for stmt in body {
            self.generate_statement(stmt)?;
        }

        // 确保有 Return
        if self.code.last() != Some(&(Opcode::Return as u8)) {
            self.emit_u8(Opcode::I32Ext as u8);
            self.emit_u8(I32Ext::Const as u8);
            self.emit_i32(0);
            self.emit_u8(Opcode::Return as u8);
        }

        self.patch_jumps();

        self.chunks.push(Chunk {
            locals: self.local_index as u16,
            upvalues: 0,
            max_stack: 16,
            code: std::mem::take(&mut self.code),
            handlers: Vec::new(),
            lines: Vec::new(),
        });

        Ok(())
    }

    fn generate_statement(&mut self, statement: &UastNode) -> Result<(), FormatError> {
        match statement {
            UastNode::Assign { name, value, .. } => {
                self.generate_expression(value)?;
                
                let index = if let Some(&idx) = self.locals.get(name) {
                    idx
                } else {
                    let idx = self.local_index;
                    self.locals.insert(name.clone(), idx);
                    self.local_index += 1;
                    idx
                };

                self.emit_u8(Opcode::StoreLocal as u8);
                self.emit_u8(index);
            }
            UastNode::List(items, _) => {
                for item in items {
                    self.generate_statement(item)?;
                }
            }
            UastNode::Intent(intent_node, _) => {
                match intent_node.intent {
                    _ => {}
                }
            }
            _ => {
                self.generate_expression(statement)?;
                self.emit_u8(Opcode::Pop as u8);
            }
        }
        Ok(())
    }

    fn generate_expression(&mut self, expression: &UastNode) -> Result<(), FormatError> {
        match expression {
            UastNode::Literal(val, _) => {
                if let Ok(num) = val.parse::<i32>() {
                    self.emit_u8(Opcode::I32Ext as u8);
                    self.emit_u8(I32Ext::Const as u8);
                    self.emit_i32(num);
                } else if val.starts_with('"') || val.starts_with('\'') {
                    let content = val.trim_matches('"').trim_matches('\'').to_string();
                    let idx = self.constants.len() as u16;
                    self.constants.push(Constant::String(content));
                    self.emit_u8(Opcode::Push as u8);
                    self.emit_u16(idx);
                } else {
                    // Identifier (Variable load)
                    if let Some(&index) = self.locals.get(val) {
                        self.emit_u8(Opcode::LoadLocal as u8);
                        self.emit_u8(index);
                    } else {
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::Const as u8);
                        self.emit_i32(0);
                    }
                }
            }
            UastNode::Call { callee, args, .. } => {
                match callee.as_str() {
                    "__if" => {
                        if args.len() < 2 { return Ok(()); }
                        let false_label = self.new_label("if_false");
                        let end_label = self.new_label("if_end");

                        // 1. Evaluate condition
                        self.generate_expression(&args[0])?;
                        
                        // 2. Jump if false
                        self.emit_jump(&false_label, true);

                        // 3. True block
                        self.generate_statement(&args[1])?;
                        self.emit_jump(&end_label, false);

                        // 4. False block
                        self.define_label(&false_label);
                        if args.len() > 2 {
                            self.generate_statement(&args[2])?;
                        }
                        self.emit_jump(&end_label, false);

                        // 5. End block
                        self.define_label(&end_label);
                        return Ok(());
                    }
                    "__while" => {
                        if args.len() < 2 { return Ok(()); }
                        let cond_label = self.new_label("while_cond");
                        let end_label = self.new_label("while_end");

                        // 1. Condition block
                        self.define_label(&cond_label);
                        self.generate_expression(&args[0])?;
                        self.emit_jump(&end_label, true);

                        // 2. Body block
                        self.generate_statement(&args[1])?;
                        self.emit_jump(&cond_label, false);

                        // 3. End block
                        self.define_label(&end_label);
                        return Ok(());
                    }
                    "add" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::Add as u8);
                    }
                    "sub" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::Sub as u8);
                    }
                    "mul" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::Mul as u8);
                    }
                    "div" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::DivS as u8);
                    }
                    "eq" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::Eq as u8);
                    }
                    "lt" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::LtS as u8);
                    }
                    "gt" => {
                        self.generate_expression(&args[0])?;
                        self.generate_expression(&args[1])?;
                        self.emit_u8(Opcode::I32Ext as u8);
                        self.emit_u8(I32Ext::GtS as u8);
                    }
                    "__return" => {
                        if !args.is_empty() {
                            self.generate_expression(&args[0])?;
                        } else {
                            self.emit_u8(Opcode::I32Ext as u8);
                            self.emit_u8(I32Ext::Const as u8);
                            self.emit_i32(0);
                        }
                        self.emit_u8(Opcode::Return as u8);
                    }
                    _ => {
                        // Function call (simplified, assumes chunk index matches order)
                        // In a real implementation, we would need to resolve the chunk index properly
                        for arg in args {
                            self.generate_expression(arg)?;
                        }
                        // Assume chunk index 0 is main, others follow
                        self.emit_u8(Opcode::Call as u8);
                        self.emit_u16(1); // placeholder for chunk index
                        self.emit_u8(args.len() as u8);
                    }
                }
            }
            _ => {
                 self.emit_u8(Opcode::I32Ext as u8);
                 self.emit_u8(I32Ext::Const as u8);
                 self.emit_i32(0);
            }
        }
        Ok(())
    }
}
