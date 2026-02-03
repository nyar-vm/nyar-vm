//! CSharp 到 Nyar 字节码的翻译器

use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::NyarError;
use oak_java::ast::*;

/// CSharp 翻译器上下文
///
/// 用于管理翻译过程中的状态，如符号表、E-Graph 构建器等。
pub struct TranslatorContext<'a> {
    pub builder: IntentBuilder<'a, ConstraintAnalysis>,
}

impl<'a> TranslatorContext<'a> {
    pub fn new(egraph: &'a mut EGraph<IKun, ConstraintAnalysis>) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
        }
    }

    /// 解析内置函数映射
    pub fn resolve_builtin(&mut self, target: &str, method: &str, args: Vec<chomsky_uir::egraph::Id>, loc: Loc) -> Option<chomsky_uir::egraph::Id> {
        match (target, method) {
            // System.Console
            ("System.Console", "WriteLine") | ("Console", "WriteLine") => {
                Some(self.builder.cross_lang_call("nyar", "std::io::println", args, loc))
            }
            ("System.Console", "Write") | ("Console", "Write") => {
                Some(self.builder.cross_lang_call("nyar", "std::io::print", args, loc))
            }
            ("System.Console", "ReadLine") | ("Console", "ReadLine") => {
                Some(self.builder.cross_lang_call("nyar", "std::io::read_line", args, loc))
            }
            // System.Math
            ("System.Math", "Abs") | ("Math", "Abs") => Some(self.builder.cross_lang_call("nyar", "std::math::abs", args, loc)),
            ("System.Math", "Sqrt") | ("Math", "Sqrt") => Some(self.builder.cross_lang_call("nyar", "std::math::sqrt", args, loc)),
            ("System.Math", "Pow") | ("Math", "Pow") => Some(self.builder.cross_lang_call("nyar", "std::math::pow", args, loc)),
            ("System.Math", "Sin") | ("Math", "Sin") => Some(self.builder.cross_lang_call("nyar", "std::math::sin", args, loc)),
            ("System.Math", "Cos") | ("Math", "Cos") => Some(self.builder.cross_lang_call("nyar", "std::math::cos", args, loc)),
            ("System.Math", "Tan") | ("Math", "Tan") => Some(self.builder.cross_lang_call("nyar", "std::math::tan", args, loc)),
            ("System.Math", "Log") | ("Math", "Log") => Some(self.builder.cross_lang_call("nyar", "std::math::log", args, loc)),
            ("System.Math", "Exp") | ("Math", "Exp") => Some(self.builder.cross_lang_call("nyar", "std::math::exp", args, loc)),
            ("System.Math", "Floor") | ("Math", "Floor") => Some(self.builder.cross_lang_call("nyar", "std::math::floor", args, loc)),
            ("System.Math", "Ceiling") | ("Math", "Ceiling") => Some(self.builder.cross_lang_call("nyar", "std::math::ceil", args, loc)),
            ("System.Math", "Round") | ("Math", "Round") => Some(self.builder.cross_lang_call("nyar", "std::math::round", args, loc)),
            ("System.Math", "Min") | ("Math", "Min") => Some(self.builder.cross_lang_call("nyar", "std::math::min", args, loc)),
            ("System.Math", "Max") | ("Math", "Max") => Some(self.builder.cross_lang_call("nyar", "std::math::max", args, loc)),
            // System.String
            ("System.String", "Concat") | ("String", "Concat") => Some(self.builder.cross_lang_call("nyar", "std::string::concat", args, loc)),
            ("System.String", "IsNullOrEmpty") | ("String", "IsNullOrEmpty") => Some(self.builder.cross_lang_call("nyar", "std::string::is_null_or_empty", args, loc)),
            // System.Convert
            ("System.Convert", "ToInt32") | ("Convert", "ToInt32") => Some(self.builder.cross_lang_call("nyar", "std::convert::to_int", args, loc)),
            ("System.Convert", "ToDouble") | ("Convert", "ToDouble") => Some(self.builder.cross_lang_call("nyar", "std::convert::to_float", args, loc)),
            ("System.Convert", "ToString") | ("Convert", "ToString") => Some(self.builder.cross_lang_call("nyar", "std::convert::to_string", args, loc)),
            // System.Environment
            ("System.Environment", "Exit") | ("Environment", "Exit") => Some(self.builder.cross_lang_call("nyar", "std::os::exit", args, loc)),
            _ => None,
        }
    }
}

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_graph(
        &self,
        ast: &JavaRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ctx = TranslatorContext::new(egraph);
        self.translate_root(ast, &mut ctx)
    }

    pub fn translate_to_tree(&self, ast: &JavaRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DefaultCostModel::default());

        Ok(extractor.extract(root_id))
    }

    fn translate_root(
        &self,
        root: &JavaRoot,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &root.items {
            match item {
                Item::Class(class) => items.push(self.translate_class(class, ctx)?),
                Item::Interface(interface) => items.push(self.translate_interface(interface, ctx)?),
                Item::Struct(struct_decl) => items.push(self.translate_struct(struct_decl, ctx)?),
                Item::Enum(enum_decl) => items.push(self.translate_enum(enum_decl, ctx)?),
                Item::Record(record) => items.push(self.translate_record(record, ctx)?),
                _ => {}
            }
        }
        Ok(ctx.builder.module("root", items))
    }

    fn translate_class(
        &self,
        class: &ClassDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &class.members {
            match member {
                Member::Method(method) => {
                    members.push(self.translate_method(method, ctx)?);
                }
                Member::Field(field) => {
                    members.push(self.translate_field(field, ctx)?);
                }
                Member::Constructor(ctor) => {
                    members.push(self.translate_method(ctor, ctx)?);
                }
            }
        }
        Ok(ctx.builder.module(&class.name, members))
    }

    fn translate_interface(
        &self,
        interface: &InterfaceDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &interface.members {
            if let Member::Method(method) = member {
                members.push(self.translate_method(method, ctx)?);
            }
        }
        Ok(ctx.builder.module(&interface.name, members))
    }

    fn translate_struct(
        &self,
        struct_decl: &StructDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &struct_decl.members {
            match member {
                Member::Method(method) => members.push(self.translate_method(method, ctx)?),
                Member::Field(field) => members.push(self.translate_field(field, ctx)?),
                Member::Constructor(ctor) => members.push(self.translate_method(ctor, ctx)?),
            }
        }
        Ok(ctx.builder.module(&struct_decl.name, members))
    }

    fn translate_enum(
        &self,
        enum_decl: &EnumDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut variants = Vec::new();
        let loc = Loc::unknown();
        for variant in &enum_decl.variants {
            // 目前简单将枚举项处理为常量符号
            variants.push(ctx.builder.export(variant, ctx.builder.constant(0, loc), loc));
        }
        Ok(ctx.builder.module(&enum_decl.name, variants))
    }

    fn translate_record(
        &self,
        record: &RecordDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        // 处理主构造函数参数为字段
        for param in &record.parameters {
            let loc = Loc::unknown();
            let field_id = ctx.builder.symbol(&param.name, loc);
            members.push(ctx.builder.export(&param.name, field_id, loc));
        }
        for member in &record.members {
            match member {
                Member::Method(method) => members.push(self.translate_method(method, ctx)?),
                Member::Field(field) => members.push(self.translate_field(field, ctx)?),
                Member::Constructor(ctor) => members.push(self.translate_method(ctor, ctx)?),
            }
        }
        Ok(ctx.builder.module(&record.name, members))
    }

    fn translate_field(
        &self,
        field: &FieldDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let name_id = ctx.builder.symbol(&field.name, loc);
        // 目前简单处理为导出符号
        Ok(ctx.builder.export(&field.name, name_id, loc))
    }

    fn translate_method(
        &self,
        method: &MethodDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let body_id = self.translate_block(&method.body, ctx)?;

        // 处理方法参数
        let mut params = Vec::new();
        for param in &method.parameters {
            params.push(param.name.clone());
        }
        let lambda = ctx.builder.lambda(params, body_id, loc);
        Ok(ctx.builder.export(&method.name, lambda, loc))
    }

    fn translate_block(
        &self,
        statements: &[Statement],
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut stmts = Vec::new();
        for stmt in statements {
            stmts.push(self.translate_stmt(stmt, ctx)?);
        }
        Ok(ctx.builder.block(stmts, Loc::unknown()))
    }

    fn translate_stmt(
        &self,
        stmt: &Statement,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match stmt {
            Statement::Expression(expr) => self.translate_expr(expr, ctx),
            Statement::Return(Some(expr)) => {
                let val = self.translate_expr(expr, ctx)?;
                Ok(ctx.builder.return_(val, loc))
            }
            Statement::Return(None) => {
                let void = ctx.builder.constant(0, loc); // Placeholder for void
                Ok(ctx.builder.return_(void, loc))
            },
            Statement::Block(inner) => self.translate_block(inner, ctx),
            Statement::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_id = self.translate_expr(condition, ctx)?;
                let then_id = self.translate_stmt(then_branch, ctx)?;
                let else_id = if let Some(eb) = else_branch {
                    self.translate_stmt(eb, ctx)?
                } else {
                    ctx.builder.seq(vec![], loc)
                };
                Ok(ctx.builder.extension("if", vec![cond_id, then_id, else_id], loc))
            }
            Statement::While { condition, body } => {
                let cond_id = self.translate_expr(condition, ctx)?;
                let body_id = self.translate_stmt(body, ctx)?;
                Ok(ctx.builder.extension("while", vec![cond_id, body_id], loc))
            }
            Statement::DoWhile { condition, body } => {
                let cond_id = self.translate_expr(condition, ctx)?;
                let body_id = self.translate_stmt(body, ctx)?;
                Ok(ctx.builder.extension("do_while", vec![cond_id, body_id], loc))
            }
            Statement::For {
                init,
                condition,
                update,
                body,
            } => {
                let init_id = if let Some(init_stmt) = init {
                    self.translate_stmt(init_stmt, ctx)?
                } else {
                    ctx.builder.seq(vec![], loc)
                };
                
                let cond_id = if let Some(cond_expr) = condition {
                    self.translate_expr(cond_expr, ctx)?
                } else {
                    ctx.builder.bool(true, loc)
                };

                let update_id = if let Some(update_expr) = update {
                    self.translate_expr(update_expr, ctx)?
                } else {
                    ctx.builder.seq(vec![], loc)
                };

                let body_id = self.translate_stmt(body, ctx)?;
                
                // for (init; cond; update) body -> { init; while(cond) { body; update; } }
                let loop_body = ctx.builder.seq(vec![body_id, update_id], loc);
                let loop_stmt = ctx.builder.extension("while", vec![cond_id, loop_body], loc);
                
                Ok(ctx.builder.seq(vec![init_id, loop_stmt], loc))
            }
            Statement::ForEach {
                item_type,
                item_name,
                iterable,
                body,
            } => {
                let iterable_id = self.translate_expr(iterable, ctx)?;
                let body_id = self.translate_stmt(body, ctx)?;
                let type_id = ctx.builder.string(item_type, loc);
                let name_id = ctx.builder.string(item_name, loc);
                
                Ok(ctx.builder.extension("foreach", vec![type_id, name_id, iterable_id, body_id], loc))
            }
            Statement::Switch {
                selector,
                cases,
                default,
            } => {
                let selector_id = self.translate_expr(selector, ctx)?;
                let mut case_ids = Vec::new();
                for case in cases {
                    let label_id = self.translate_expr(&case.label, ctx)?;
                    let mut stmts = Vec::new();
                    for s in &case.body {
                        stmts.push(self.translate_stmt(s, ctx)?);
                    }
                    let body_id = ctx.builder.seq(stmts, loc);
                    case_ids.push(ctx.builder.extension("case", vec![label_id, body_id], loc));
                }
                
                let default_id = if let Some(stmts) = default {
                    let mut def_stmts = Vec::new();
                    for s in stmts {
                        def_stmts.push(self.translate_stmt(s, ctx)?);
                    }
                    ctx.builder.seq(def_stmts, loc)
                } else {
                    ctx.builder.seq(vec![], loc)
                };
                
                let cases_seq = ctx.builder.seq(case_ids, loc);
                Ok(ctx.builder.extension("switch", vec![selector_id, cases_seq, default_id], loc))
            }
            Statement::Break => Ok(ctx.builder.extension("break", vec![], loc)),
            Statement::Continue => Ok(ctx.builder.extension("continue", vec![], loc)),
            Statement::Try(try_stmt) => {
                let try_block = self.translate_block(&try_stmt.block, ctx)?;
                let mut catches = Vec::new();
                for catch in &try_stmt.catches {
                    let param_name = ctx.builder.string(&catch.parameter.name, loc);
                    let param_type = ctx.builder.string(&catch.parameter.r#type, loc);
                    let block = self.translate_block(&catch.block, ctx)?;
                    catches.push(ctx.builder.extension("catch", vec![param_type, param_name, block], loc));
                }
                let catches_seq = ctx.builder.seq(catches, loc);
                
                let finally_block = if let Some(stmts) = &try_stmt.finally {
                     self.translate_block(stmts, ctx)?
                } else {
                    ctx.builder.seq(vec![], loc)
                };
                
                Ok(ctx.builder.extension("try", vec![try_block, catches_seq, finally_block], loc))
            }
            Statement::Throw(expr) => {
                let expr_id = self.translate_expr(expr, ctx)?;
                Ok(ctx.builder.extension("throw", vec![expr_id], loc))
            }
            Statement::LocalVariable { r#type, name, initializer } => {
                 let name_id = ctx.builder.string(name, loc);
                 let type_id = ctx.builder.string(r#type, loc);
                 let init_id = if let Some(init) = initializer {
                     self.translate_expr(init, ctx)?
                 } else {
                     ctx.builder.constant(0, loc)
                 };
                 Ok(ctx.builder.extension("local_variable", vec![name_id, type_id, init_id], loc))
            }
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match expr {
            Expression::Literal(Literal::Integer(v)) => Ok(ctx.builder.constant(*v, loc)),
            Expression::Literal(Literal::String(s)) => Ok(ctx.builder.string(s, loc)),
            Expression::Literal(Literal::Boolean(b)) => Ok(ctx.builder.bool(*b, loc)),
            Expression::Identifier(s) => {
                // 检查是否是内置函数全称
                if let Some(builtin) = ctx.resolve_builtin(s, "", vec![], loc) {
                    return Ok(builtin);
                }
                Ok(ctx.builder.symbol(s, loc))
            }
            Expression::MethodCall(call) => {
                let mut arg_ids = Vec::new();
                for arg in &call.arguments {
                    arg_ids.push(self.translate_expr(arg, ctx)?);
                }

                // 处理内置函数
                if let Some(target) = &call.target {
                    let target_name = self.expr_to_string(target);
                    if let Some(builtin) = ctx.resolve_builtin(&target_name, &call.name, arg_ids.clone(), loc) {
                        return Ok(builtin);
                    }
                } else {
                    if let Some(builtin) = ctx.resolve_builtin("", &call.name, arg_ids.clone(), loc) {
                        return Ok(builtin);
                    }
                }

                let name_id = ctx.builder.symbol(&call.name, loc);
                if let Some(target) = &call.target {
                    let target_id = self.translate_expr(target, ctx)?;
                    let args_id = ctx.builder.seq(arg_ids, loc);
                    Ok(ctx.builder.extension("call", vec![target_id, name_id, args_id], loc))
                } else {
                    Ok(ctx.builder.call(name_id, arg_ids, loc))
                }
            }
            Expression::FieldAccess(access) => {
                let target_id = self.translate_expr(&access.target, ctx)?;
                let name_id = ctx.builder.symbol(&access.name, loc);
                Ok(ctx.builder.extension("field", vec![target_id, name_id], loc))
            }
            Expression::Binary { left, op, right } => {
                let left_id = self.translate_expr(left, ctx)?;
                let right_id = self.translate_expr(right, ctx)?;
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
                    _ => op,
                };
                Ok(ctx.builder.extension(op_name, vec![left_id, right_id], loc))
            }
            Expression::Unary { op, expression } => {
                let expr_id = self.translate_expr(expression, ctx)?;
                let op_name = match op.as_str() {
                    "-" => "neg",
                    "!" => "not",
                    _ => op,
                };
                Ok(ctx.builder.extension(op_name, vec![expr_id], loc))
            }
            Expression::Assignment { left, op, right } => {
                let mut val = self.translate_expr(right, ctx)?;
                if op != "=" {
                    let left_val = self.translate_expr(left, ctx)?;
                    let base_op = &op[..op.len() - 1];
                    let op_name = match base_op {
                        "+" => "add",
                        "-" => "sub",
                        "*" => "mul",
                        "/" => "div",
                        "%" => "rem",
                        _ => base_op,
                    };
                    val = ctx.builder.extension(op_name, vec![left_val, val], loc);
                }

                match &**left {
                    Expression::Identifier(name) => Ok(ctx.builder.assign(name, val, loc)),
                    _ => {
                        let target = self.translate_expr(left, ctx)?;
                        Ok(ctx.builder.assign_to_id(target, val, loc))
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

                let one = ctx.builder.constant(1, loc);
                let current_val = ctx.builder.symbol(&name, loc);
                let new_val = ctx.builder.extension(op_name, vec![current_val, one], loc);
                let assign = ctx.builder.assign(&name, new_val, loc);

                if *is_prefix {
                    Ok(ctx.builder.seq(vec![assign, ctx.builder.symbol(&name, loc)], loc))
                } else {
                    let temp_name = format!("_tmp_{}", name);
                    let save_old = ctx.builder.assign(&temp_name, current_val, loc);
                    let return_old = ctx.builder.symbol(&temp_name, loc);
                    Ok(ctx.builder.seq(vec![save_old, assign, return_old], loc))
                }
            }
            Expression::New(new_expr) => {
                let type_id = ctx.builder.string(&new_expr.r#type, loc);
                let mut arg_ids = Vec::new();
                for arg in &new_expr.arguments {
                    arg_ids.push(self.translate_expr(arg, ctx)?);
                }
                let args_id = ctx.builder.seq(arg_ids, loc);
                Ok(ctx.builder.extension("new", vec![type_id, args_id], loc))
            }
            Expression::ArrayCreation(ac) => {
                let type_id = ctx.builder.string(&ac.element_type, loc);
                let mut dim_ids = Vec::new();
                for dim in &ac.dimensions {
                    dim_ids.push(self.translate_expr(dim, ctx)?);
                }
                let dims_id = ctx.builder.seq(dim_ids, loc);
                Ok(ctx.builder.extension("new_array", vec![type_id, dims_id], loc))
            }
            Expression::ArrayAccess(aa) => {
                let target_id = self.translate_expr(&aa.target, ctx)?;
                let index_id = self.translate_expr(&aa.index, ctx)?;
                Ok(ctx.builder.extension("array_access", vec![target_id, index_id], loc))
            }
            Expression::This => Ok(ctx.builder.symbol("this", loc)),
            Expression::Super => Ok(ctx.builder.symbol("super", loc)),
        }
    }

    fn expr_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(s) => s.clone(),
            Expression::FieldAccess(fa) => {
                let target = self.expr_to_string(&fa.target);
                format!("{}.{}", target, fa.name)
            }
            _ => "".to_string(),
        }
    }
}
