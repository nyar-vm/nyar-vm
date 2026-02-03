//! CSharp 到 Nyar 字节码的翻译器

use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::NyarError;
use oak_csharp::ast::*;

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
        ast: &CSharpRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ctx = TranslatorContext::new(egraph);
        self.translate_root(ast, &mut ctx)
    }

    pub fn translate_to_tree(&self, ast: &CSharpRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DefaultCostModel::default());

        Ok(extractor.extract(root_id))
    }

    fn translate_root(
        &self,
        root: &CSharpRoot,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &root.items {
            if let Some(id) = self.translate_item(item, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.module("root", items))
    }

    fn translate_item(
        &self,
        item: &Item,
        ctx: &mut TranslatorContext,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        match item {
            Item::Namespace(ns) => Ok(Some(self.translate_namespace(ns, ctx)?)),
            Item::Using(_using) => {
                // TODO: 处理 Using 指令，将其映射到符号表的导入
                Ok(None)
            }
            Item::Class(class) => Ok(Some(self.translate_class(class, ctx)?)),
            Item::Interface(interface) => Ok(Some(self.translate_interface(interface, ctx)?)),
            Item::Struct(struct_decl) => Ok(Some(self.translate_struct(struct_decl, ctx)?)),
            Item::Enum(enum_decl) => Ok(Some(self.translate_enum(enum_decl, ctx)?)),
        }
    }

    fn translate_namespace(
        &self,
        ns: &NamespaceDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &ns.items {
            if let Some(id) = self.translate_item(item, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.module(&ns.name, items))
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
                Member::Property(prop) => {
                    // TODO: 处理属性
                    let loc = Loc::unknown();
                    let name_id = ctx.builder.symbol(&prop.name, loc);
                    members.push(ctx.builder.export(&prop.name, name_id, loc));
                }
                _ => {}
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
                _ => {}
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
        for variant in &enum_decl.members {
            // 目前简单将枚举项处理为常量符号
            variants.push(ctx.builder.export(variant, ctx.builder.constant(0, loc), loc));
        }
        Ok(ctx.builder.module(&enum_decl.name, variants))
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
        let body_id = if let Some(body) = &method.body {
            self.translate_block(body, ctx)?
        } else {
            ctx.builder.seq(vec![], loc)
        };

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
            Statement::Foreach {
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
            Statement::Break => Ok(ctx.builder.extension("break", vec![], loc)),
            Statement::Continue => Ok(ctx.builder.extension("continue", vec![], loc)),
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
            Expression::Literal(Literal::Null) => Ok(ctx.builder.constant(0, loc)),
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
            Expression::MemberAccess(access) => {
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
                    "~" => "bit_not",
                    _ => op,
                };
                Ok(ctx.builder.extension(op_name, vec![expr_id], loc))
            }
            Expression::Assignment { left, op, right } => {
                let left_id = self.translate_expr(left, ctx)?;
                let right_id = self.translate_expr(right, ctx)?;
                Ok(ctx.builder.extension("assign", vec![left_id, right_id], loc))
            }
            Expression::Await(expr) => {
                let expr_id = self.translate_expr(expr, ctx)?;
                Ok(ctx.builder.extension("await", vec![expr_id], loc))
            }
            _ => Ok(ctx.builder.constant(0, loc)),
        }
    }

    fn expr_to_string(&self, expr: &Expression) -> String {
        match expr {
            Expression::Identifier(s) => s.clone(),
            Expression::MemberAccess(access) => format!("{}.{}", self.expr_to_string(&access.target), access.name),
            _ => String::new(),
        }
    }
}
