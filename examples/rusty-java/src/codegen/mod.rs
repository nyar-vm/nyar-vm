//! Java 到 Nyar UIR 的转换器

use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder, Id};
use nyar_types::NyarError;
use oak_java::ast::*;

/// Java 到 UIR 的转换器
pub struct JavaUirConverter<'a> {
    builder: IntentBuilder<'a, ConstraintAnalysis>,
    source_id: u32,
}

impl<'a> JavaUirConverter<'a> {
    /// 创建新的转换器
    pub fn new(egraph: &'a mut EGraph<IKun, ConstraintAnalysis>, source_id: u32) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
            source_id,
        }
    }

    /// 辅助方法：创建位置信息
    fn loc(&self) -> Loc {
        Loc::new(self.source_id, 0, 0)
    }
}

impl JavaUirConverter<'_> {
    /// 将 Java AST 转换为 UIR 树
    pub fn convert_to_tree(ast: &JavaRoot, source_id: u32) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        {
            let mut converter = JavaUirConverter::new(&mut egraph, source_id);
            let root_id = converter.convert_root(ast)?;

            if let Some(root_id) = root_id {
                let extractor = chomsky_extract::IKunExtractor::new(
                    &egraph,
                    chomsky_cost::DEFAULT_COST_MODEL.clone(),
                );
                return Ok(extractor.extract(root_id));
            }
        }
        Err(NyarError::Compile("No code generated".to_string()))
    }

    /// 转换根节点
    pub fn convert_root(&mut self, root: &JavaRoot) -> Result<Option<Id>, NyarError> {
        let mut items = Vec::new();
        for item in &root.items {
            match item {
                Item::Class(class) => {
                    items.push(self.convert_class(class)?);
                }
                Item::Interface(interface) => {
                    items.push(self.convert_interface(interface)?);
                }
                Item::Package(pkg) => {
                    let name_id = self.builder.string(&pkg.name, self.loc());
                    items.push(self.builder.extension("package", vec![name_id], self.loc()));
                }
                Item::Import(imp) => {
                    let path_id = self.builder.string(&imp.path, self.loc());
                    let is_static_id = self.builder.bool(imp.is_static, self.loc());
                    items.push(self.builder.extension("import", vec![path_id, is_static_id], self.loc()));
                }
            }
        }
        if items.is_empty() {
            Ok(None)
        } else {
            Ok(Some(self.builder.seq(items, self.loc())))
        }
    }

    fn convert_class(&mut self, class: &ClassDeclaration) -> Result<Id, NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            match member {
                Member::Method(method) => {
                    members.push(self.convert_method(method)?);
                }
                Member::Field(field) => {
                    members.push(self.convert_field(field)?);
                }
                Member::Constructor(ctor) => {
                    members.push(self.convert_constructor(ctor)?);
                }
            }
        }
        let name_id = self.builder.string(&class.name, self.loc());
        let members_id = self.builder.seq(members, self.loc());

        let mut modifiers = Vec::new();
        for m in &class.modifiers {
            modifiers.push(self.builder.string(m, self.loc()));
        }
        let modifiers_id = self.builder.seq(modifiers, self.loc());

        let extends_id = if let Some(ext) = &class.extends {
            self.builder.string(ext, self.loc())
        } else {
            self.builder.constant(0, self.loc()) // Use 0 or null as placeholder
        };

        let mut implements = Vec::new();
        for i in &class.implements {
            implements.push(self.builder.string(i, self.loc()));
        }
        let implements_id = self.builder.seq(implements, self.loc());

        Ok(self.builder.extension(
            "class",
            vec![name_id, modifiers_id, extends_id, implements_id, members_id],
            self.loc(),
        ))
    }

    fn convert_interface(&mut self, interface: &InterfaceDeclaration) -> Result<Id, NyarError> {
        let mut members = Vec::new();
        for member in &interface.members {
            match member {
                Member::Method(method) => {
                    members.push(self.convert_method(method)?);
                }
                Member::Field(field) => {
                    members.push(self.convert_field(field)?);
                }
                Member::Constructor(ctor) => {
                    members.push(self.convert_constructor(ctor)?);
                }
            }
        }
        let name_id = self.builder.string(&interface.name, self.loc());
        let members_id = self.builder.seq(members, self.loc());

        let mut modifiers = Vec::new();
        for m in &interface.modifiers {
            modifiers.push(self.builder.string(m, self.loc()));
        }
        let modifiers_id = self.builder.seq(modifiers, self.loc());

        let mut extends = Vec::new();
        for e in &interface.extends {
            extends.push(self.builder.string(e, self.loc()));
        }
        let extends_id = self.builder.seq(extends, self.loc());

        Ok(self.builder.extension(
            "interface",
            vec![name_id, modifiers_id, extends_id, members_id],
            self.loc(),
        ))
    }

    fn convert_field(&mut self, field: &FieldDeclaration) -> Result<Id, NyarError> {
        let name_id = self.builder.string(&field.name, self.loc());
        let type_id = self.builder.string(&field.r#type, self.loc());

        let mut modifiers = Vec::new();
        for m in &field.modifiers {
            modifiers.push(self.builder.string(m, self.loc()));
        }
        let modifiers_id = self.builder.seq(modifiers, self.loc());

        Ok(self.builder.extension("field", vec![name_id, type_id, modifiers_id], self.loc()))
    }

    fn convert_constructor(&mut self, ctor: &ConstructorDeclaration) -> Result<Id, NyarError> {
        let body_id = self.convert_block(&ctor.body)?;
        let name_id = self.builder.string(&ctor.name, self.loc());

        let mut param_ids = Vec::new();
        for param in &ctor.parameters {
            param_ids.push(self.convert_parameter(param)?);
        }
        let params_id = self.builder.seq(param_ids, self.loc());

        let mut modifiers = Vec::new();
        for m in &ctor.modifiers {
            modifiers.push(self.builder.string(m, self.loc()));
        }
        let modifiers_id = self.builder.seq(modifiers, self.loc());

        Ok(self.builder.extension(
            "constructor",
            vec![name_id, modifiers_id, params_id, body_id],
            self.loc(),
        ))
    }

    fn convert_method(&mut self, method: &MethodDeclaration) -> Result<Id, NyarError> {
        let body_id = self.convert_block(&method.body)?;
        let name_id = self.builder.string(&method.name, self.loc());
        let ret_id = self.builder.string(&method.return_type, self.loc());

        let mut param_ids = Vec::new();
        for param in &method.parameters {
            param_ids.push(self.convert_parameter(param)?);
        }
        let params_id = self.builder.seq(param_ids, self.loc());

        let mut modifiers = Vec::new();
        for m in &method.modifiers {
            modifiers.push(self.builder.string(m, self.loc()));
        }
        let modifiers_id = self.builder.seq(modifiers, self.loc());

        // 规范化 method 扩展：[name, modifiers, params, return_type, body]
        Ok(self.builder.extension(
            "method",
            vec![name_id, modifiers_id, params_id, ret_id, body_id],
            self.loc(),
        ))
    }

    fn convert_parameter(&mut self, param: &Parameter) -> Result<Id, NyarError> {
        let name_id = self.builder.string(&param.name, self.loc());
        let type_id = self.builder.string(&param.r#type, self.loc());
        Ok(self.builder.extension("parameter", vec![name_id, type_id], self.loc()))
    }

    fn convert_block(&mut self, stmts: &[Statement]) -> Result<Id, NyarError> {
        let mut ids = Vec::new();
        for stmt in stmts {
            ids.push(self.convert_stmt(stmt)?);
        }
        Ok(self.builder.seq(ids, self.loc()))
    }

    fn convert_stmt(&mut self, stmt: &Statement) -> Result<Id, NyarError> {
        match stmt {
            Statement::Expression(expr) => self.convert_expr(expr),
            Statement::Return(Some(expr)) => {
                let val = self.convert_expr(expr)?;
                Ok(self.builder.extension("return", vec![val], self.loc()))
            }
            Statement::Return(None) => Ok(self.builder.extension("return", vec![], self.loc())),
            Statement::Block(stmts) => self.convert_block(stmts),
            Statement::LocalVariable {
                r#type,
                name,
                initializer,
            } => {
                let name_id = self.builder.string(name, self.loc());
                let type_id = self.builder.string(r#type, self.loc());
                let init_id = if let Some(init) = initializer {
                    self.convert_expr(init)?
                } else {
                    self.builder.constant(0, self.loc())
                };
                Ok(self.builder.extension(
                    "local_variable",
                    vec![name_id, type_id, init_id],
                    self.loc(),
                ))
            }
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_id = self.convert_expr(condition)?;
                let then_id = self.convert_stmt(then_branch)?;
                let else_id = if let Some(eb) = else_branch {
                    self.convert_stmt(eb)?
                } else {
                    self.builder.seq(vec![], self.loc())
                };
                Ok(self.builder.branch(cond_id, then_id, else_id, self.loc()))
            }
            Statement::While { condition, body } => {
                let cond_id = self.convert_expr(condition)?;
                let body_id = self.convert_stmt(body)?;
                Ok(self.builder.while_loop(cond_id, body_id, self.loc()))
            }
            Statement::DoWhile { condition, body } => {
                let cond_id = self.convert_expr(condition)?;
                let body_id = self.convert_stmt(body)?;
                Ok(self.builder.extension("do_while", vec![cond_id, body_id], self.loc()))
            }
            Statement::For {
                init,
                condition,
                update,
                body,
            } => {
                let init_id = if let Some(i) = init {
                    self.convert_stmt(i)?
                } else {
                    self.builder.seq(vec![], self.loc())
                };
                let cond_id = if let Some(c) = condition {
                    self.convert_expr(c)?
                } else {
                    self.builder.bool(true, self.loc())
                };
                let update_id = if let Some(u) = update {
                    self.convert_expr(u)?
                } else {
                    self.builder.seq(vec![], self.loc())
                };
                let body_id = self.convert_stmt(body)?;
                Ok(self.builder.extension("for", vec![init_id, cond_id, update_id, body_id], self.loc()))
            }
            Statement::ForEach {
                item_type,
                item_name,
                iterable,
                body,
            } => {
                let type_id = self.builder.string(item_type, self.loc());
                let name_id = self.builder.string(item_name, self.loc());
                let iterable_id = self.convert_expr(iterable)?;
                let body_id = self.convert_stmt(body)?;
                Ok(self.builder.extension("foreach", vec![type_id, name_id, iterable_id, body_id], self.loc()))
            }
            Statement::Switch {
                selector,
                cases,
                default,
            } => {
                let selector_id = self.convert_expr(selector)?;
                let mut case_ids = Vec::new();
                for case in cases {
                    let label_id = self.convert_expr(&case.label)?;
                    let body_id = self.convert_block(&case.body)?;
                    case_ids.push(self.builder.extension("case", vec![label_id, body_id], self.loc()));
                }
                let cases_id = self.builder.seq(case_ids, self.loc());
                let default_id = if let Some(d) = default {
                    self.convert_block(d)?
                } else {
                    self.builder.seq(vec![], self.loc())
                };
                Ok(self.builder.extension("switch", vec![selector_id, cases_id, default_id], self.loc()))
            }
            Statement::Break => Ok(self.builder.extension("break", vec![], self.loc())),
            Statement::Continue => Ok(self.builder.extension("continue", vec![], self.loc())),
            Statement::Try(try_stmt) => {
                let block_id = self.convert_block(&try_stmt.block)?;
                let mut catch_ids = Vec::new();
                for catch in &try_stmt.catches {
                    let param_id = self.convert_parameter(&catch.parameter)?;
                    let catch_body_id = self.convert_block(&catch.block)?;
                    catch_ids.push(self.builder.extension("catch", vec![param_id, catch_body_id], self.loc()));
                }
                let catches_id = self.builder.seq(catch_ids, self.loc());
                let finally_id = if let Some(f) = &try_stmt.finally {
                    self.convert_block(f)?
                } else {
                    self.builder.seq(vec![], self.loc())
                };
                Ok(self.builder.extension("try", vec![block_id, catches_id, finally_id], self.loc()))
            }
            Statement::Throw(expr) => {
                let expr_id = self.convert_expr(expr)?;
                Ok(self.builder.extension("throw", vec![expr_id], self.loc()))
            }
        }
    }

    fn convert_expr(&mut self, expr: &Expression) -> Result<Id, NyarError> {
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(self.builder.constant(*v, self.loc())),
            Expression::Literal(Literal::String(s)) => Ok(self.builder.string(s, self.loc())),
            Expression::Literal(Literal::Boolean(b)) => Ok(self.builder.bool(*b, self.loc())),
            Expression::Identifier(s) => {
                // 特殊处理 System.out
                if s == "System.out" {
                    let system = self.builder.symbol("System", self.loc());
                    let out = self.builder.symbol("out", self.loc());
                    return Ok(self.builder.extension("get_field", vec![system, out], self.loc()));
                }
                Ok(self.builder.symbol(s, self.loc()))
            }
            Expression::FieldAccess(fa) => {
                let target = self.convert_expr(&fa.target)?;
                let name = self.builder.symbol(&fa.name, self.loc());
                Ok(self.builder.extension("get_field", vec![target, name], self.loc()))
            }
            Expression::Binary { left, op, right } => {
                let left_id = self.convert_expr(left)?;
                let right_id = self.convert_expr(right)?;
                let op_name = match op.as_str() {
                    "+" => "add",
                    "-" => "sub",
                    "*" => "mul",
                    "/" => "div",
                    "%" => "rem",
                    "==" => "eq",
                    "!=" => "ne",
                    "<" => "lt",
                    "<=" => "le",
                    ">" => "gt",
                    ">=" => "ge",
                    "&&" => "and",
                    "||" => "or",
                    "&" => "bit_and",
                    "|" => "bit_or",
                    "^" => "bit_xor",
                    "<<" => "shl",
                    ">>" => "shr",
                    ">>>" => "ushr",
                    _ => op,
                };
                Ok(self.builder.extension(op_name, vec![left_id, right_id], self.loc()))
            }
            Expression::Unary { op, expression } => {
                let expr_id = self.convert_expr(expression)?;
                let op_name = match op.as_str() {
                    "-" => "neg",
                    "!" => "not",
                    "~" => "bit_not",
                    _ => op,
                };
                Ok(self.builder.extension(op_name, vec![expr_id], self.loc()))
            }
            Expression::Assignment { left, op, right } => {
                let mut val = self.convert_expr(right)?;
                if op != "=" {
                    // 处理复合赋值，如 x += y 转换为 x = x + y
                    let left_val = self.convert_expr(left)?;
                    let base_op = &op[..op.len() - 1];
                    let op_name = match base_op {
                        "+" => "add",
                        "-" => "sub",
                        "*" => "mul",
                        "/" => "div",
                        "%" => "rem",
                        "&" => "bit_and",
                        "|" => "bit_or",
                        "^" => "bit_xor",
                        "<<" => "shl",
                        ">>" => "shr",
                        ">>>" => "ushr",
                        _ => base_op,
                    };
                    val = self.builder.extension(op_name, vec![left_val, val], self.loc());
                }

                match &**left {
                    Expression::Identifier(name) => Ok(self.builder.assign(name, val, self.loc())),
                    _ => {
                        let target = self.convert_expr(left)?;
                        Ok(self.builder.assign_to_id(target, val, self.loc()))
                    }
                }
            }
            Expression::Update { expression, op, is_prefix } => {
                let name = if let Expression::Identifier(name) = &**expression {
                    name.clone()
                } else {
                    return Err(NyarError::Compile("Increment/decrement only supported for identifiers".to_string()));
                };

                let op_name = match op.as_str() {
                    "++" => "add",
                    "--" => "sub",
                    _ => return Err(NyarError::Compile(format!("Unknown update operator: {}", op))),
                };

                let one = self.builder.constant(1, self.loc());
                let current_val = self.builder.symbol(&name, self.loc());
                let new_val = self.builder.extension(op_name, vec![current_val, one], self.loc());
                let assign = self.builder.assign(&name, new_val, self.loc());

                if *is_prefix {
                    // ++x: (x = x + 1, x)
                    Ok(self.builder.seq(vec![assign, self.builder.symbol(&name, self.loc())], self.loc()))
                } else {
                    // x++: (old = x, x = x + 1, old)
                    // Note: This is a bit complex in UIR without temp vars.
                    // For now, let's just do the assignment and return the new value for simplicity,
                    // or implement it properly if UIR supports let-bindings.
                    // Since we are in a compiler, we can probably use a temporary variable if needed.
                    // But wait, UIR's `seq` returns the value of the last expression.
                    
                    // A better way for x++:
                    // we need to return the old value.
                    // Let's assume we can use a temporary variable name that won't conflict.
                    let temp_name = format!("_tmp_{}", name);
                    let save_old = self.builder.assign(&temp_name, current_val, self.loc());
                    let return_old = self.builder.symbol(&temp_name, self.loc());
                    Ok(self.builder.seq(vec![save_old, assign, return_old], self.loc()))
                }
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.convert_expr(arg)?);
                }

                // 特殊处理标准库方法
                if let Some(target) = &call.target {
                    match &**target {
                        // System.out.println / print
                        Expression::Identifier(s) if s == "System.out" => {
                            if call.name == "println" {
                                return Ok(self.builder.cross_lang_call("nyar", "std::io", "println", arg_ids, self.loc()));
                            } else if call.name == "print" {
                                return Ok(self.builder.cross_lang_call("nyar", "std::io", "print", arg_ids, self.loc()));
                            }
                        }
                        Expression::FieldAccess(fa) => {
                            if let Expression::Identifier(t) = &*fa.target {
                                if t == "System" && fa.name == "out" {
                                    if call.name == "println" {
                                        return Ok(self.builder.cross_lang_call("nyar", "std::io", "println", arg_ids, self.loc()));
                                    } else if call.name == "print" {
                                        return Ok(self.builder.cross_lang_call("nyar", "std::io", "print", arg_ids, self.loc()));
                                    }
                                }
                            }
                        }
                        // Math.xxx
                        Expression::Identifier(s) if s == "Math" => {
                            match call.name.as_str() {
                                "sqrt" => return Ok(self.builder.cross_lang_call("nyar", "math", "sqrt", arg_ids, self.loc())),
                                "abs" => return Ok(self.builder.cross_lang_call("nyar", "math", "abs", arg_ids, self.loc())),
                                "sin" => return Ok(self.builder.cross_lang_call("nyar", "math", "sin", arg_ids, self.loc())),
                                "cos" => return Ok(self.builder.cross_lang_call("nyar", "math", "cos", arg_ids, self.loc())),
                                "tan" => return Ok(self.builder.cross_lang_call("nyar", "math", "tan", arg_ids, self.loc())),
                                "random" => return Ok(self.builder.cross_lang_call("nyar", "math", "rand", arg_ids, self.loc())),
                                _ => {}
                            }
                        }
                        // System.currentTimeMillis
                        Expression::Identifier(s) if s == "System" => {
                            if call.name == "currentTimeMillis" {
                                return Ok(self.builder.cross_lang_call("nyar", "time", "now", arg_ids, self.loc()));
                            }
                        }
                        _ => {}
                    }
                }

                let name_id = self.builder.symbol(&call.name, self.loc());
                let args_id = self.builder.seq(arg_ids, self.loc());
                if let Some(target) = &call.target {
                    let target_id = self.convert_expr(target)?;
                    Ok(self.builder.extension(
                        "call",
                        vec![target_id, name_id, args_id],
                        self.loc(),
                    ))
                } else {
                    Ok(self.builder.extension("call", vec![name_id, args_id], self.loc()))
                }
            }
            Expression::New(new_expr) => {
                let type_id = self.builder.string(&new_expr.r#type, self.loc());
                let mut arg_ids = Vec::new();
                for arg in &new_expr.arguments {
                    arg_ids.push(self.convert_expr(arg)?);
                }
                let args_id = self.builder.seq(arg_ids, self.loc());
                Ok(self.builder.extension("new", vec![type_id, args_id], self.loc()))
            }
            Expression::This => Ok(self.builder.symbol("this", self.loc())),
            Expression::Super => Ok(self.builder.symbol("super", self.loc())),
            Expression::ArrayAccess(access) => {
                let target = self.convert_expr(&access.target)?;
                let index = self.convert_expr(&access.index)?;
                Ok(self.builder.extension("get_element", vec![target, index], self.loc()))
            }
            Expression::ArrayCreation(creation) => {
                let type_id = self.builder.string(&creation.element_type, self.loc());
                let mut dim_ids = Vec::new();
                for dim in &creation.dimensions {
                    dim_ids.push(self.convert_expr(dim)?);
                }
                let dims_id = self.builder.seq(dim_ids, self.loc());
                Ok(self.builder.extension("array_new", vec![type_id, dims_id], self.loc()))
            }
            Expression::Ternary { condition, then_branch, else_branch } => {
                let cond_id = self.convert_expr(condition)?;
                let then_id = self.convert_expr(then_branch)?;
                let else_id = self.convert_expr(else_branch)?;
                Ok(self.builder.branch(cond_id, then_id, else_id, self.loc()))
            }
            Expression::Cast { target_type, expression } => {
                let expr_id = self.convert_expr(expression)?;
                let type_id = self.builder.string(target_type, self.loc());
                Ok(self.builder.extension("cast", vec![expr_id, type_id], self.loc()))
            }
        }
    }
}
