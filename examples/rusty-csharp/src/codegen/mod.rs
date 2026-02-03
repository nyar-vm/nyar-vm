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
            Item::Using(using) => {
                // TODO: 将 Using 指令映射到 Nyar 的导入系统
                let loc = Loc::unknown();
                let path_id = ctx.builder.symbol(&using.path, loc);
                Ok(Some(ctx.builder.extension("import", vec![path_id], loc)))
            }
            Item::Class(class) => Ok(Some(self.translate_class(class, ctx)?)),
            Item::Interface(interface) => Ok(Some(self.translate_interface(interface, ctx)?)),
            Item::Struct(struct_decl) => Ok(Some(self.translate_struct(struct_decl, ctx)?)),
            Item::Enum(enum_decl) => Ok(Some(self.translate_enum(enum_decl, ctx)?)),
            Item::Record(record_decl) => Ok(Some(self.translate_record(record_decl, ctx)?)),
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
                    members.push(self.translate_property(prop, ctx)?);
                }
                Member::Indexer(indexer) => {
                    members.push(self.translate_indexer(indexer, ctx)?);
                }
                Member::Event(event) => {
                    members.push(self.translate_event(event, ctx)?);
                }
            }
        }
        // 处理泛型参数
        if !class.type_parameters.is_empty() {
            // TODO: 在 Nyar 中支持泛型类
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

    pub fn translate_struct(
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

    pub fn translate_record(
        &self,
        record_decl: &RecordDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut members = Vec::new();
        for member in &record_decl.members {
            match member {
                Member::Method(method) => members.push(self.translate_method(method, ctx)?),
                Member::Field(field) => members.push(self.translate_field(field, ctx)?),
                Member::Constructor(ctor) => members.push(self.translate_method(ctor, ctx)?),
                Member::Property(prop) => members.push(self.translate_property(prop, ctx)?),
                _ => {}
            }
        }
        Ok(ctx.builder.module(&record_decl.name, members))
    }

    fn translate_enum(
        &self,
        enum_decl: &EnumDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut variants = Vec::new();
        let loc = Loc::unknown();
        for variant in &enum_decl.members {
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
        Ok(ctx.builder.export(&field.name, name_id, loc))
    }

    fn translate_property(
        &self,
        prop: &PropertyDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let mut accessors = Vec::new();
        if let Some(get) = &prop.get_accessor {
            let body_id = if let Some(body) = &get.body {
                self.translate_block(body, ctx)?
            } else {
                ctx.builder.seq(vec![], loc)
            };
            let get_id = ctx.builder.lambda(vec![], body_id, loc);
            accessors.push(ctx.builder.export(&format!("get_{}", prop.name), get_id, loc));
        }
        if let Some(set) = &prop.set_accessor {
            let body_id = if let Some(body) = &set.body {
                self.translate_block(body, ctx)?
            } else {
                ctx.builder.seq(vec![], loc)
            };
            let set_id = ctx.builder.lambda(vec!["value".to_string()], body_id, loc);
            accessors.push(ctx.builder.export(&format!("set_{}", prop.name), set_id, loc));
        }
        Ok(ctx.builder.module(&prop.name, accessors))
    }

    fn translate_indexer(
        &self,
        indexer: &IndexerDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let mut accessors = Vec::new();
        let mut params = Vec::new();
        for p in &indexer.parameters {
            params.push(p.name.clone());
        }

        if let Some(get) = &indexer.get_accessor {
            let body_id = if let Some(body) = &get.body {
                self.translate_block(body, ctx)?
            } else {
                ctx.builder.seq(vec![], loc)
            };
            let get_id = ctx.builder.lambda(params.clone(), body_id, loc);
            accessors.push(ctx.builder.export("get_Item", get_id, loc));
        }
        if let Some(set) = &indexer.set_accessor {
            let body_id = if let Some(body) = &set.body {
                self.translate_block(body, ctx)?
            } else {
                ctx.builder.seq(vec![], loc)
            };
            let mut set_params = params.clone();
            set_params.push("value".to_string());
            let set_id = ctx.builder.lambda(set_params, body_id, loc);
            accessors.push(ctx.builder.export("set_Item", set_id, loc));
        }
        Ok(ctx.builder.module("Indexer", accessors))
    }

    fn translate_event(
        &self,
        event: &EventDeclaration,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        let name_id = ctx.builder.symbol(&event.name, loc);
        Ok(ctx.builder.export(&event.name, name_id, loc))
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

        let mut params = Vec::new();
        for param in &method.parameters {
            params.push(param.name.clone());
        }
        
        let mut lambda = ctx.builder.lambda(params, body_id, loc);
        if method.is_async {
            lambda = ctx.builder.extension("async", vec![lambda], loc);
        }
        
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
            Statement::Return(expr) => {
                let expr_id = if let Some(e) = expr {
                    self.translate_expr(e, ctx)?
                } else {
                    ctx.builder.constant(0, loc)
                };
                Ok(ctx.builder.extension("return", vec![expr_id], loc))
            }
            Statement::Block(stmts) => self.translate_block(stmts, ctx),
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
                let mut stmts = Vec::new();
                if let Some(i) = init {
                    stmts.push(self.translate_stmt(i, ctx)?);
                }
                let cond_id = if let Some(c) = condition {
                    self.translate_expr(c, ctx)?
                } else {
                    ctx.builder.constant(1, loc) // true
                };
                let body_id = self.translate_stmt(body, ctx)?;
                let update_id = if let Some(u) = update {
                    self.translate_expr(u, ctx)?
                } else {
                    ctx.builder.seq(vec![], loc)
                };
                let loop_id = ctx.builder.extension("for", vec![cond_id, body_id, update_id], loc);
                stmts.push(loop_id);
                Ok(ctx.builder.seq(stmts, loc))
            }
            Statement::Foreach {
                item_type: _,
                item_name,
                iterable,
                body,
            } => {
                let iter_id = self.translate_expr(iterable, ctx)?;
                let item_id = ctx.builder.symbol(item_name, loc);
                let body_id = self.translate_stmt(body, ctx)?;
                Ok(ctx.builder.extension("foreach", vec![iter_id, item_id, body_id], loc))
            }
            Statement::LocalVariable {
                r#type: _,
                name,
                initializer,
            } => {
                let init_id = if let Some(init) = initializer {
                    self.translate_expr(init, ctx)?
                } else {
                    ctx.builder.constant(0, loc)
                };
                Ok(ctx.builder.export(name, init_id, loc))
            }
            Statement::Break => Ok(ctx.builder.extension("break", vec![], loc)),
            Statement::Continue => Ok(ctx.builder.extension("continue", vec![], loc)),
        }
    }

    fn translate_expr(
        &self,
        expr: &Expression,
        ctx: &mut TranslatorContext,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::unknown();
        match expr {
            Expression::Literal(lit) => match lit {
                Literal::Integer(n) => Ok(ctx.builder.constant(*n, loc)),
                Literal::String(s) => Ok(ctx.builder.constant(s.clone(), loc)),
                Literal::Boolean(b) => Ok(ctx.builder.constant(*b, loc)),
                Literal::Null => Ok(ctx.builder.constant(0, loc)),
            },
            Expression::Identifier(id) => Ok(ctx.builder.symbol(id, loc)),
            Expression::MethodCall(call) => {
                let mut args = Vec::new();
                for arg in &call.arguments {
                    args.push(self.translate_expr(arg, ctx)?);
                }
                if let Some(target) = &call.target {
                    if let Expression::Identifier(target_name) = target.as_ref() {
                        if let Some(builtin) = ctx.resolve_builtin(target_name, &call.name, args.clone(), loc) {
                            return Ok(builtin);
                        }
                    }
                    let target_id = self.translate_expr(target, ctx)?;
                    Ok(ctx.builder.call(target_id, args, loc))
                } else {
                    let name_id = ctx.builder.symbol(&call.name, loc);
                    Ok(ctx.builder.call(name_id, args, loc))
                }
            }
            Expression::MemberAccess(access) => {
                let target_id = self.translate_expr(&access.target, ctx)?;
                Ok(ctx.builder.get_member(target_id, &access.name, loc))
            }
            Expression::ElementAccess(access) => {
                let target_id = self.translate_expr(&access.target, ctx)?;
                let mut args = Vec::new();
                for arg in &access.arguments {
                    args.push(self.translate_expr(arg, ctx)?);
                }
                Ok(ctx.builder.call(target_id, args, loc))
            }
            Expression::New(new_expr) => {
                let mut args = Vec::new();
                for arg in &new_expr.arguments {
                    args.push(self.translate_expr(arg, ctx)?);
                }
                Ok(ctx.builder.cross_lang_call("nyar", &format!("new::{}", new_expr.r#type), args, loc))
            }
            Expression::This => Ok(ctx.builder.symbol("this", loc)),
            Expression::Base => Ok(ctx.builder.symbol("base", loc)),
            Expression::Binary { left, op, right } => {
                let left_id = self.translate_expr(left, ctx)?;
                let right_id = self.translate_expr(right, ctx)?;
                Ok(ctx.builder.binary(op, left_id, right_id, loc))
            }
            Expression::Unary { op, expression } => {
                let expr_id = self.translate_expr(expression, ctx)?;
                Ok(ctx.builder.unary(op, expr_id, loc))
            }
            Expression::Assignment { left, op, right } => {
                let left_id = self.translate_expr(left, ctx)?;
                let right_id = self.translate_expr(right, ctx)?;
                Ok(ctx.builder.binary(op, left_id, right_id, loc))
            }
            Expression::Await(expr) => {
                let expr_id = self.translate_expr(expr, ctx)?;
                Ok(ctx.builder.extension("await", vec![expr_id], loc))
            }
        }
    }
}
