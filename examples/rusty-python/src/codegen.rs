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
    /// 当前类名
    current_class: Option<String>,
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
            current_class: None,
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
                } else if let IKunTree::Extension(name, args) = &**value {
                    if name == "python_function" {
                        // args = [name_str, lambda, defaults, vararg, kwarg]
                        if let IKunTree::StringConstant(func_name) = &args[0] {
                            if let IKunTree::Lambda(params, body) = &args[1] {
                                function_defs.push((func_name.clone(), params.clone(), body));
                                continue;
                            }
                        }
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

        self.current_class = Some(name.clone());

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
                    } else if let IKunTree::Extension(name, args) = &**value {
                        if name == "python_function" {
                            // args = [name_str, lambda, defaults, vararg, kwarg]
                            if let IKunTree::Lambda(params, body) = &args[1] {
                                if let IKunTree::Symbol(method_name) = &**target {
                                    let method =
                                        self.generate_function_from_tree(method_name, params, body)?;
                                    methods.push(method);
                                    continue;
                                }
                            }
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

        self.current_class = None;
        self.defined_classes.insert(name.clone());

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
                    "dict_item" | "dict_unpack" => {
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
                    "list_comp" | "set_comp" | "dict_comp" | "generator_exp" => {
                        let is_dict = name == "dict_comp";
                        let method = match name.as_str() {
                            "list_comp" => "list",
                            "set_comp" => "set",
                            "dict_comp" => "dict",
                            "generator_exp" => "generator",
                            _ => unreachable!(),
                        };
                        let append_method = match name.as_str() {
                            "list_comp" => "append",
                            "set_comp" => "add",
                            "dict_comp" => "__setitem__",
                            "generator_exp" => "yield",
                            _ => unreachable!(),
                        };

                        // 1. Create the container
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: method.to_string(),
                                signature: GaiaSignature {
                                    params: vec![],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));

                        // 2. Store in a temporary local
                        let result_index = self.local_index;
                        self.local_types.push(GaiaType::Object);
                        self.local_index += 1;
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::StoreLocal(result_index, GaiaType::Object),
                        ));

                        // 3. Generate loops
                        if is_dict {
                            let key = &args[0];
                            let value = &args[1];
                            let generators = &args[2..];
                            self.generate_nested_dict_generator(
                                generators,
                                key,
                                value,
                                result_index,
                                append_method,
                            )?;
                        } else {
                            let elt = &args[0];
                            let generators = &args[1..];
                            self.generate_nested_generator(
                                generators,
                                elt,
                                result_index,
                                append_method,
                            )?;
                        }

                        // 4. Load the result back
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::LoadLocal(result_index, GaiaType::Object),
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
                            let class_name = self.current_class.clone().unwrap_or_else(|| "Object".to_string());
                            self.current_instructions.push(GaiaInstruction::Core(
                                CoreInstruction::LoadField(class_name, field_name.clone()),
                            ));
                        }
                    }
                    "set_field" => {
                        self.generate_tree_node(&args[0], false)?; // object
                        self.generate_tree_node(&args[2], false)?; // value
                        if let IKunTree::Symbol(field_name) = &args[1] {
                            let class_name = self.current_class.clone().unwrap_or_else(|| "Object".to_string());
                            self.current_instructions.push(GaiaInstruction::Core(
                                CoreInstruction::StoreField(class_name, field_name.clone()),
                            ));
                        }
                    }
                    "keyword_arg" => {
                        self.generate_tree_node(&args[0], false)?; // name (usually StringConstant)
                        self.generate_tree_node(&args[1], false)?; // value
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "keyword_arg".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 2],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "python_call" => {
                        self.generate_tree_node(&args[0], false)?; // func
                        self.generate_tree_node(&args[1], false)?; // args_seq
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "python_call".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 2],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "python_function" => {
                        let name = if let IKunTree::StringConstant(s) = &args[0] {
                            s.clone()
                        } else {
                            "lambda".to_string()
                        };
                        let lam = &args[1];
                        let defaults = &args[2];
                        let vararg = &args[3];
                        let kwarg = &args[4];

                        // Push name
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::PushConstant(GaiaConstant::String(name.clone())),
                        ));
                        // Generate lambda
                        self.generate_tree_node(lam, false)?;
                        // Generate defaults
                        self.generate_tree_node(defaults, false)?;
                        // Generate vararg
                        self.generate_tree_node(vararg, false)?;
                        // Generate kwarg
                        self.generate_tree_node(kwarg, false)?;

                        // Call Builtins.make_function(name, lam, defaults, vararg, kwarg)
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "make_function".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object; 5],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "async" => {
                        self.generate_tree_node(&args[0], false)?;
                        self.current_instructions.push(GaiaInstruction::Managed(
                            ManagedInstruction::CallMethod {
                                target: "Builtins".to_string(),
                                method: "make_async".to_string(),
                                signature: GaiaSignature {
                                    params: vec![GaiaType::Object],
                                    return_type: GaiaType::Object,
                                },
                                is_virtual: false,
                            },
                        ));
                    }
                    "none" => {
                        self.current_instructions.push(GaiaInstruction::Core(
                            CoreInstruction::PushConstant(GaiaConstant::Null),
                        ));
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

                if let IKunTree::Symbol(name) = &**func {
                    if self.defined_classes.contains(name) {
                        // 类实例化: p = Person(args)
                        // 1. 创建对象并保留一份在栈底作为返回值
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
                }

                // 普通函数或闭包调用
                for arg in args {
                    self.generate_tree_node(arg, false)?;
                }
                if let IKunTree::Symbol(name) = &**func {
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
                } else {
                    self.generate_tree_node(func, false)?;
                    self.current_instructions.push(GaiaInstruction::Managed(
                        ManagedInstruction::CallMethod {
                            target: "Closure".to_string(),
                            method: "call".to_string(),
                            signature: GaiaSignature {
                                params: vec![GaiaType::Object; args.len()],
                                return_type: GaiaType::Object,
                            },
                            is_virtual: true,
                        },
                    ));
                }
            }
            IKunTree::CrossLangCall {
                language,
                module_path,
                function_name,
                arguments,
            } => {
                for arg in arguments {
                    self.generate_tree_node(arg, false)?;
                }
                let target = format!("{}.{}", language, module_path.replace("::", "."));
                self.current_instructions.push(GaiaInstruction::Managed(
                    ManagedInstruction::CallStatic {
                        target,
                        method: function_name.clone(),
                        signature: GaiaSignature {
                            params: vec![GaiaType::Object; arguments.len()],
                            return_type: GaiaType::Object,
                        },
                    },
                ));
            }
            _ => {}
        }
        Ok(())
    }

    /// 生成嵌套生成器循环
    fn generate_nested_generator(
        &mut self,
        generators: &[IKunTree],
        elt: &IKunTree,
        result_index: u32,
        append_method: &str,
    ) -> Result<(), GaiaError> {
        if generators.is_empty() {
            // 最内层：评估元素并添加到容器
            self.current_instructions.push(GaiaInstruction::Core(
                CoreInstruction::LoadLocal(result_index, GaiaType::Object),
            ));
            self.generate_tree_node(elt, false)?;
            self.current_instructions.push(GaiaInstruction::Managed(
                ManagedInstruction::CallMethod {
                    target: "Object".to_string(),
                    method: append_method.to_string(),
                    signature: GaiaSignature {
                        params: vec![GaiaType::Object],
                        return_type: GaiaType::Void,
                    },
                    is_virtual: true,
                },
            ));
            return Ok(());
        }

        self.generate_comprehension_loop(generators, elt, None, result_index, append_method)
    }

    /// 生成嵌套字典生成器循环
    fn generate_nested_dict_generator(
        &mut self,
        generators: &[IKunTree],
        key: &IKunTree,
        value: &IKunTree,
        result_index: u32,
        append_method: &str,
    ) -> Result<(), GaiaError> {
        if generators.is_empty() {
            // 最内层：评估键值对并添加到字典
            self.current_instructions.push(GaiaInstruction::Core(
                CoreInstruction::LoadLocal(result_index, GaiaType::Object),
            ));
            self.generate_tree_node(key, false)?;
            self.generate_tree_node(value, false)?;
            self.current_instructions.push(GaiaInstruction::Managed(
                ManagedInstruction::CallMethod {
                    target: "Object".to_string(),
                    method: append_method.to_string(),
                    signature: GaiaSignature {
                        params: vec![GaiaType::Object, GaiaType::Object],
                        return_type: GaiaType::Void,
                    },
                    is_virtual: true,
                },
            ));
            return Ok(());
        }

        self.generate_comprehension_loop(generators, key, Some(value), result_index, append_method)
    }

    /// 生成单个推导式循环
    fn generate_comprehension_loop(
        &mut self,
        generators: &[IKunTree],
        elt: &IKunTree,
        elt_value: Option<&IKunTree>,
        result_index: u32,
        append_method: &str,
    ) -> Result<(), GaiaError> {
        let gen = &generators[0];
        if let IKunTree::Extension(name, gen_args) = gen {
            if name == "comprehension" {
                let target = &gen_args[0];
                let iter = &gen_args[1];
                let ifs = &gen_args[2];

                let test_label = self.new_label("comp_test");
                let body_label = self.new_label("comp_body");
                let end_label = self.new_label("comp_end");

                // 获取迭代器
                self.generate_tree_node(iter, false)?;
                self.current_instructions.push(GaiaInstruction::Managed(
                    ManagedInstruction::CallMethod {
                        target: "Builtins".to_string(),
                        method: "iter".to_string(),
                        signature: GaiaSignature {
                            params: vec![GaiaType::Object],
                            return_type: GaiaType::Object,
                        },
                        is_virtual: false,
                    },
                ));

                let iter_index = self.local_index;
                self.local_types.push(GaiaType::Object);
                self.local_index += 1;
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::StoreLocal(iter_index, GaiaType::Object),
                ));

                self.finish_block(GaiaTerminator::Jump(test_label.clone()));
                self.start_block(test_label.clone());

                // 获取下一个值
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::LoadLocal(iter_index, GaiaType::Object),
                ));
                self.current_instructions.push(GaiaInstruction::Managed(
                    ManagedInstruction::CallMethod {
                        target: "Builtins".to_string(),
                        method: "next".to_string(),
                        signature: GaiaSignature {
                            params: vec![GaiaType::Object],
                            return_type: GaiaType::Object,
                        },
                        is_virtual: false,
                    },
                ));

                // 检查是否结束
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::Dup,
                ));
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::PushConstant(GaiaConstant::Null),
                ));
                self.current_instructions.push(GaiaInstruction::Core(
                    CoreInstruction::Cmp(CmpCondition::Ne, GaiaType::Object),
                ));

                self.finish_block(GaiaTerminator::Branch {
                    true_label: body_label.clone(),
                    false_label: end_label.clone(),
                });

                self.start_block(body_label);

                // 赋值给目标
                if let IKunTree::Symbol(name) = target {
                    let target_index = if let Some(&idx) = self.locals.get(name) {
                        idx
                    } else {
                        let idx = self.local_index;
                        self.locals.insert(name.clone(), idx);
                        self.local_types.push(GaiaType::Object);
                        self.local_index += 1;
                        idx
                    };
                    self.current_instructions.push(GaiaInstruction::Core(
                        CoreInstruction::StoreLocal(target_index, GaiaType::Object),
                    ));
                } else {
                    self.current_instructions.push(GaiaInstruction::Core(CoreInstruction::Pop));
                }

                // 处理条件过滤 (ifs)
                if let IKunTree::Seq(if_list) = ifs {
                    for if_cond in if_list {
                        let if_true_label = self.new_label("comp_if_true");
                        self.generate_tree_node(if_cond, false)?;
                        self.finish_block(GaiaTerminator::Branch {
                            true_label: if_true_label.clone(),
                            false_label: test_label.clone(),
                        });
                        self.start_block(if_true_label);
                    }
                }

                // 递归处理下一层
                if let Some(val) = elt_value {
                    self.generate_nested_dict_generator(&generators[1..], elt, val, result_index, append_method)?;
                } else {
                    self.generate_nested_generator(&generators[1..], elt, result_index, append_method)?;
                }

                self.finish_block(GaiaTerminator::Jump(test_label));
                self.start_block(end_label);
            }
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
