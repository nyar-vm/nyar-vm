//! Gaia 指令生成器
//!
//! 将 UIR (Universal Intermediate Representation) 转换为 Gaia 指令

use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use chomsky_types::ChomskyResult;
use chomsky_uir::{EGraph, Id};
use gaia_assembler::{
    instruction::{CmpCondition, CoreInstruction, GaiaInstruction, ManagedInstruction},
    program::{GaiaBlock, GaiaConstant, GaiaFunction, GaiaModule, GaiaTerminator},
    types::{GaiaSignature, GaiaType},
};
use gaia_types::{GaiaError, SourceLocation};
use std::collections::HashMap;

/// Gaia 翻译器，将 UIR 转换为 Gaia 指令
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
    /// 已定义的类名
    defined_classes: std::collections::HashSet<String>,
}

impl Backend for GaiaTranslator {
    fn name(&self) -> &str {
        "gaia"
    }

    fn generate(&self, tree: &IKunTree) -> ChomskyResult<BackendArtifact> {
        let mut translator = GaiaTranslator::new();
        let module = translator.generate_from_tree(tree).map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("Gaia error: {:?}", e))
        })?;
        let json = serde_json::to_string_pretty(&module).map_err(|e| {
            chomsky_types::ChomskyError::backend_error(format!("Serialization error: {:?}", e))
        })?;
        Ok(BackendArtifact::Source(json))
    }
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
            defined_classes: std::collections::HashSet::new(),
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

    /// 从意图树生成 GaiaModule
    pub fn generate_from_tree(&mut self, tree: &IKunTree) -> Result<GaiaModule, GaiaError> {
        let mut functions = Vec::new();

        // 1. 提取顶级元素
        let (module_name, items) = match tree {
            IKunTree::Module(name, items) => (name.as_str(), items.as_slice()),
            IKunTree::Seq(items) => ("mini_python_program", items.as_slice()),
            IKunTree::Extension(name, args) if name == "python_module" => {
                ("mini_python_program", &args[1..])
            }
            _ => ("mini_python_program", std::slice::from_ref(tree)),
        };

        // 2. 分离函数、类定义和主语句
        let mut main_stmts = Vec::new();
        let mut function_defs = Vec::new();
        let mut class_defs = Vec::new();

        for item in items {
            if let IKunTree::StateUpdate(target, value) = item {
                if let IKunTree::Lambda(params, body) = &**value {
                    if let IKunTree::Symbol(name) = &**target {
                        function_defs.push((name.clone(), params.clone(), body));
                        continue;
                    }
                }
            }
            if let IKunTree::Extension(name, args) = item {
                if name == "class_def" {
                    if let IKunTree::Symbol(class_name) = &args[0] {
                        self.defined_classes.insert(class_name.clone());
                    }
                    class_defs.push(args);
                    continue;
                }
            }
            main_stmts.push(item);
        }

        // 3. 生成主函数
        if !main_stmts.is_empty() {
            let main_function = self.generate_main_function_from_tree(main_stmts)?;
            functions.push(main_function);
        }

        // 4. 生成其他函数
        for (name, params, body) in function_defs {
            let function = self.generate_function_from_tree(&name, &params, body)?;
            functions.push(function);
        }

        // 5. 生成类
        let mut classes = Vec::new();
        for args in class_defs {
            let class = self.generate_class_from_tree(args)?;
            classes.push(class);
        }

        Ok(GaiaModule {
            name: module_name.to_string(),
            functions,
            structs: Vec::new(),
            classes,
            constants: self.string_constants.clone(),
            globals: Vec::new(),
            imports: Vec::new(),
        })
    }

    fn generate_main_function_from_tree(
        &mut self,
        statements: Vec<&IKunTree>,
    ) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();
        for stmt in statements {
            self.generate_tree_node(stmt, true)?;
        }
        self.finish_block(GaiaTerminator::Return);

        Ok(GaiaFunction {
            name: "main".to_string(),
            signature: GaiaSignature {
                params: Vec::new(),
                return_type: GaiaType::Void,
            },
            blocks: std::mem::take(&mut self.blocks),
            is_external: false,
        })
    }

    fn generate_function_from_tree(
        &mut self,
        name: &str,
        parameters: &[String],
        body: &IKunTree,
    ) -> Result<GaiaFunction, GaiaError> {
        self.reset_for_function();
        for (i, param_name) in parameters.iter().enumerate() {
            self.locals.insert(param_name.clone(), i as u32);
            self.local_types.push(GaiaType::Object);
            self.local_index += 1;
        }
        self.generate_tree_node(body, true)?;
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

    fn generate_class_from_tree(
        &mut self,
        args: &[IKunTree],
    ) -> Result<gaia_assembler::program::GaiaClass, GaiaError> {
        let name = if let IKunTree::Symbol(name) = &args[0] {
            name.clone()
        } else {
            return Err(GaiaError::syntax_error(
                "Class name must be a symbol",
                SourceLocation::default(),
            ));
        };

        let mut parent = None;
        if let IKunTree::Extension(ext_name, bases) = &args[1] {
            if ext_name == "bases" && !bases.is_empty() {
                if let IKunTree::Symbol(base_name) = &bases[0] {
                    parent = Some(base_name.clone());
                }
            }
        }

        let mut methods = Vec::new();
        let mut fields = Vec::new();

        if let IKunTree::Seq(body_items) = &args[2] {
            for item in body_items {
                if let IKunTree::StateUpdate(target, value) = item {
                    if let IKunTree::Lambda(params, body) = &**value {
                        if let IKunTree::Symbol(name) = &**target {
                            let method = self.generate_function_from_tree(name, params, body)?;
                            methods.push(method);
                            continue;
                        }
                    }
                    if let IKunTree::Symbol(name) = &**target {
                        fields.push(gaia_assembler::program::GaiaField {
                            name: name.clone(),
                            ty: GaiaType::Object,
                            is_static: true,
                            visibility: gaia_assembler::program::Visibility::Public,
                        });
                    }
                }
            }
        }

        Ok(gaia_assembler::program::GaiaClass {
            name,
            parent,
            interfaces: Vec::new(),
            fields,
            methods,
            attributes: Vec::new(),
        })
    }

    fn generate_tree_node(
        &mut self,
        tree: &IKunTree,
        _is_statement: bool,
    ) -> Result<(), GaiaError> {
        match tree {
            IKunTree::Constant(v) => {
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::PushConstant(GaiaConstant::I64(*v)),
                ));
            }
            IKunTree::FloatConstant(bits) => {
                let f = f64::from_bits(*bits);
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::PushConstant(GaiaConstant::F64(f)),
                ));
            }
            IKunTree::BooleanConstant(b) => {
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::PushConstant(GaiaConstant::Bool(*b)),
                ));
            }
            IKunTree::StringConstant(s) => {
                let const_name = format!("str_{}", self.string_constants.len());
                let constant = GaiaConstant::String(s.clone());
                self.string_constants
                    .push((const_name.clone(), constant.clone()));
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::PushConstant(constant),
                ));
            }
            IKunTree::Symbol(name) => {
                if let Some(&local_index) = self.locals.get(name) {
                    self.current_instructions.push(GaiaInstruction::Core(
                        CoreInstruction::LoadLocal(local_index, GaiaType::Object),
                    ));
                } else {
                    return Err(GaiaError::syntax_error(
                        format!("Undefined variable: {}", name),
                        SourceLocation::default(),
                    ));
                }
            }
            IKunTree::StateUpdate(target, value) => {
                self.generate_tree_node(value, false)?;
                if let IKunTree::Symbol(name) = &**target {
                    let local_index = if let Some(&index) = self.locals.get(name) {
                        index
                    } else {
                        let index = self.local_index;
                        self.locals.insert(name.clone(), index);
                        self.local_types.push(GaiaType::Object);
                        self.local_index += 1;
                        index
                    };
                    self.current_instructions.push(GaiaInstruction::Core(
                        CoreInstruction::StoreLocal(local_index, GaiaType::Object),
                    ));
                }
            }
            IKunTree::Seq(items) => {
                for item in items {
                    self.generate_tree_node(item, true)?;
                }
            }
            IKunTree::Choice(cond, then_branch, else_branch) => {
                let true_label = self.new_label("if_true");
                let false_label = self.new_label("if_false");
                let end_label = self.new_label("if_end");

                self.generate_tree_node(cond, false)?;
                self.finish_block(GaiaTerminator::Branch {
                    true_label: true_label.clone(),
                    false_label: false_label.clone(),
                });

                self.start_block(true_label);
                self.generate_tree_node(then_branch, true)?;
                self.finish_block(GaiaTerminator::Jump(end_label.clone()));

                self.start_block(false_label);
                self.generate_tree_node(else_branch, true)?;
                self.finish_block(GaiaTerminator::Jump(end_label.clone()));

                self.start_block(end_label);
            }
            IKunTree::Extension(name, args) => {
                match name.as_str() {
                    "while" => {
                        let test_label = self.new_label("while_test");
                        let body_label = self.new_label("while_body");
                        let end_label = self.new_label("while_end");

                        self.finish_block(GaiaTerminator::Jump(test_label.clone()));
                        self.start_block(test_label.clone());
                        self.generate_tree_node(&args[0], false)?;
                        self.finish_block(GaiaTerminator::Branch {
                            true_label: body_label.clone(),
                            false_label: end_label.clone(),
                        });

                        self.start_block(body_label);
                        self.generate_tree_node(&args[1], true)?;
                        self.finish_block(GaiaTerminator::Jump(test_label));

                        self.start_block(end_label);
                    }
                    "return" => {
                        if !args.is_empty() {
                            self.generate_tree_node(&args[0], false)?;
                        } else {
                            self.current_instructions.push(GaiaInstruction::Core(
                                CoreInstruction::PushConstant(GaiaConstant::Null),
                            ));
                        }
                        self.finish_block(GaiaTerminator::Return);
                        let label = self.new_label("unreachable");
                        self.start_block(label);
                    }
                    "add" | "sub" | "mul" | "div" | "mod" | "lshift" | "rshift" | "bitor" | "bitxor" | "bitand" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        let instr = match name.as_str() {
                            "add" => CoreInstruction::Add(GaiaType::Object),
                            "sub" => CoreInstruction::Sub(GaiaType::Object),
                            "mul" => CoreInstruction::Mul(GaiaType::Object),
                            "div" => CoreInstruction::Div(GaiaType::Object),
                            "mod" => CoreInstruction::Rem(GaiaType::Object),
                            "lshift" => CoreInstruction::Shl(GaiaType::Object),
                            "rshift" => CoreInstruction::Shr(GaiaType::Object),
                            "bitor" => CoreInstruction::Or(GaiaType::Object),
                            "bitxor" => CoreInstruction::Xor(GaiaType::Object),
                            "bitand" => CoreInstruction::And(GaiaType::Object),
                            _ => unreachable!(),
                        };
                        self.current_instructions.push(GaiaInstruction::Core(instr));
                    }
                    "floordiv" | "pow" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: name.clone(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 2],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "eq" | "noteq" | "lt" | "lte" | "gt" | "gte" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        let cond = match name.as_str() {
                            "eq" => CmpCondition::Eq,
                            "noteq" => CmpCondition::Ne,
                            "lt" => CmpCondition::Lt,
                            "lte" => CmpCondition::Le,
                            "gt" => CmpCondition::Gt,
                            "gte" => CmpCondition::Ge,
                            _ => unreachable!(),
                        };
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::Cmp(cond, GaiaType::Object),
                        ));
                    }
                    "is" | "isnot" | "in" | "notin" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: name.clone(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 2],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "invert" | "not" | "uadd" | "usub" => {
                        self.generate_tree_node(&args[0], false)?;
                        let instr = match name.as_str() {
                            "invert" => CoreInstruction::Not(GaiaType::Object),
                            "not" => CoreInstruction::Not(GaiaType::Object),
                            "uadd" => return Ok(()), // no-op
                            "usub" => CoreInstruction::Neg(GaiaType::Object),
                            _ => unreachable!(),
                        };
                        self.current_instructions.push(GaiaInstruction::Core(instr));
                    }
                    "and" | "or" => {
                        let end_label = self.new_label("logical_end");
                        let is_and = name == "and";

                        for (i, arg) in args.iter().enumerate() {
                            self.generate_tree_node(arg, false)?;

                            if i < args.len() - 1 {
                                self.current_instructions.push(GaiaInstruction::Core(
                                    CoreInstruction::Dup,
                                ));
                                let next_label = self.new_label("logical_next");

                                if is_and {
                                    self.finish_block(GaiaTerminator::Branch {
                                        true_label: next_label.clone(),
                                        false_label: end_label.clone(),
                                    });
                                } else {
                                    self.finish_block(GaiaTerminator::Branch {
                                        true_label: end_label.clone(),
                                        false_label: next_label.clone(),
                                    });
                                }

                                self.start_block(next_label);
                                self.current_instructions.push(GaiaInstruction::Core(
                                    CoreInstruction::Pop,
                                ));
                            }
                        }

                        self.finish_block(GaiaTerminator::Jump(end_label.clone()));
                        self.start_block(end_label);
                    }
                    "list" | "tuple" | "set" => {
                        for arg in args {
                            self.generate_tree_node(arg, false)?;
                        }
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: name.clone(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; args.len()],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "dict" => {
                        for arg in args {
                            self.generate_tree_node(arg, false)?;
                        }
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "dict".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; args.len()],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "dict_item" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "dict_item".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 2],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "list_comp" | "set_comp" | "dict_comp" | "generator_exp" => {
                        // FIXME: implement comprehensions
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::PushConstant(GaiaConstant::Null),
                        ));
                    }
                    "slice" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        self.generate_tree_node(&args[2], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "slice".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 3],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "subscript" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.generate_tree_node(&args[1], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Object".to_string(),
                                method: "__getitem__".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 1],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: true,
                            },
                        ));
                    }
                    "bytes" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "bytes".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "fstring" => {
                        for arg in args {
                            self.generate_tree_node(arg, false)?;
                        }
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "fstring".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; args.len()],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "formatted_value" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "format".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "starred" | "starred_double" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: name.clone(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "get_field" => {
                        self.generate_tree_node(&args[0], false)?;
                        if let IKunTree::Symbol(field_name) = &args[1] {
                            self.current_instructions.push(GaiaInstruction::Core(
                                CoreInstruction::LoadField("Object".to_string(), field_name.clone()),
                            ));
                        }
                    }
                    "set_field" => {
                        self.generate_tree_node(&args[0], false)?; // object
                        self.generate_tree_node(&args[2], false)?; // value
                        if let IKunTree::Symbol(field_name) = &args[1] {
                            self.current_instructions.push(GaiaInstruction::Core(
                                CoreInstruction::StoreField("Object".to_string(), field_name.clone()),
                            ));
                        }
                    }
                    _ => {}
                }
            }
            IKunTree::Apply(func, args) => {
                if let IKunTree::Extension(ext_name, ext_args) = &**func {
                    if ext_name == "get_field" {
                        // 方法调用: obj.method(args)
                        self.generate_tree_node(&ext_args[0], false)?; // push obj
                        for arg in args {
                            self.generate_tree_node(arg, false)?;
                        }
                        if let IKunTree::Symbol(method_name) = &ext_args[1] {
                            self.current_instructions.push(GaiaInstruction::Managed(
                                ManagedInstruction::CallMethod {
                                    target: "Object".to_string(),
                                    method: method_name.clone(),
                                    signature: GaiaSignature {
                                        params: vec![GaiaType::Object; args.len()],
                                        return_type: GaiaType::Object,
                                    },
                                    is_virtual: true,
                                },
                            ));
                        }
                        return Ok(());
                    }
                }

                for arg in args {
                    self.generate_tree_node(arg, false)?;
                }
                if let IKunTree::Symbol(name) = &**func {
                    if self.defined_classes.contains(name) {
                        // 类实例化: p = Person(args)
                        // 1. 创建对象
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::New(name.clone()),
                        ));
                        // 2. 复制对象引用以便调用 __init__
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::Dup,
                        ));
                        // 3. 将参数移动到对象引用之后 (Gaia 调用约定)
                        // 注意：这里可能需要调整栈顺序，或者简单地假设 __init__ 接受 self + args
                        // 在 Gaia 中，CallMethod 通常期望 [obj, arg1, arg2, ...]
                        // 现在的栈是 [..., obj, obj, arg1, arg2, ...]
                        // 我们需要把第二个 obj 移到 args 后面，或者在 push args 之前 push obj
                        
                        // 重新实现实例化逻辑以匹配栈顺序
                        self.current_instructions.pop(); // 移除刚才 push 的两个
                        self.current_instructions.pop();
                        
                        // 1. 创建对象并保留在栈底作为返回值
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::New(name.clone()),
                        ));
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::Dup,
                        ));
                        // 2. 准备参数
                        for arg in args {
                            self.generate_tree_node(arg, false)?;
                        }
                        // 3. 调用 __init__
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: name.clone(),
                                method: "__init__".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; args.len()],
                                    return_type: GaiaType::Void,
                                },
                                is_virtual: false,
                            },
                        ));
                        return Ok(());
                    }

                    self.current_instructions.push(GaiaInstruction::Managed(
                        ManagedInstruction::CallStatic {
                            target: "global".to_string(),
                            method: name.clone(),
                            signature: GaiaSignature {
                                params: vec![GaiaType::Object; args.len()],
                                return_type: GaiaType::Object,
                            },
                        },
                    ));
                }
            }
            IKunTree::CrossLangCall(lang, name, args) => {
                for arg in args {
                    self.generate_tree_node(arg, false)?;
                }
                if lang == "nyar" && name == "std::io::println" {
                    self.current_instructions.push(GaiaInstruction::Managed(
                        ManagedInstruction::CallStatic {
                            target: "nyar.std.io".to_string(),
                            method: "println".to_string(),
                            signature: GaiaSignature {
                                params: vec![GaiaType::Object; args.len()],
                                return_type: GaiaType::Object,
                            },
                        },
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// 生成 GaiaModule (Legacy)
    pub fn generate(
        &mut self,
        _egraph: &EGraph<chomsky_uir::IKun>,
        _root: Id,
    ) -> Result<GaiaModule, GaiaError> {
        // ... (existing implementation or delegate to tree-based one if possible)
        // For simplicity, I'll just leave it or remove it since I'm refactoring.
        // I'll keep it for now but it's not the preferred way.
        Err(GaiaError::syntax_error(
            "Deprecated: Use generate_from_tree instead",
            SourceLocation::default(),
        ))
    }
}
