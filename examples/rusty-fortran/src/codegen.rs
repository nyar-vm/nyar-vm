//! Fortran 到 Nyar 字节码的翻译器

use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::{Loc, NyarError};
use oak_fortran::ast::*;

/// Fortran 翻译器上下文
///
/// 用于管理翻译过程中的状态，如符号表、E-Graph 构建器等。
pub struct TranslatorContext<'a, A: chomsky_uir::Analysis<IKun> = ()> {
    pub builder: IntentBuilder<'a, A>,
}

impl<'a, A: chomsky_uir::Analysis<IKun>> TranslatorContext<'a, A> {
    pub fn new(egraph: &'a mut EGraph<IKun, A>) -> Self {
        Self {
            builder: IntentBuilder::new(egraph),
        }
    }

    pub fn new_with_builder(builder: IntentBuilder<'a, A>) -> Self {
        Self { builder }
    }
}

pub struct NyarTranslator;

impl NyarTranslator {
    pub fn new() -> Self {
        Self
    }

    pub fn translate_to_graph(
        &self,
        ast: &FortranRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ctx = TranslatorContext::new(egraph);
        self.translate_root(ast, &mut ctx)
    }

    pub fn translate_to_tree(&self, ast: &FortranRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DefaultCostModel::default());

        Ok(extractor.extract(root_id))
    }

    pub fn translate_root<A: chomsky_uir::Analysis<IKun>>(
        &self,
        root: &FortranRoot,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for unit in &root.units {
            if let Some(id) = self.translate_program_unit(unit, ctx)? {
                items.push(id);
            }
        }
        let name = root.name.as_deref().unwrap_or("root");
        Ok(ctx.builder.module(name, items, Loc::default()))
    }

    fn translate_program_unit<A: chomsky_uir::Analysis<IKun>>(
        &self,
        unit: &ProgramUnitKind,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        match unit {
            ProgramUnitKind::MainProgram(main) => Ok(Some(self.translate_main_program(main, ctx)?)),
            ProgramUnitKind::Subroutine(sub) => Ok(Some(self.translate_subroutine(sub, ctx)?)),
            ProgramUnitKind::Function(func) => Ok(Some(self.translate_function(func, ctx)?)),
            ProgramUnitKind::Module(module) => Ok(Some(self.translate_module(module, ctx)?)),
            ProgramUnitKind::Submodule(submodule) => Ok(Some(self.translate_submodule(submodule, ctx)?)),
            ProgramUnitKind::BlockData(block_data) => Ok(Some(self.translate_block_data(block_data, ctx)?)),
        }
    }

    fn translate_main_program<A: chomsky_uir::Analysis<IKun>>(
        &self,
        main: &MainProgramNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        // 翻译 specification_part
        for spec in &main.specification_part {
            if let Some(id) = self.translate_specification_stmt(spec, ctx)? {
                items.push(id);
            }
        }
        // 翻译 execution_part
        for exec in &main.execution_part {
            items.push(self.translate_executable_stmt(exec, ctx)?);
        }
        // 翻译 internal_subprograms
        for sub in &main.internal_subprograms {
            if let Some(id) = self.translate_program_unit(sub, ctx)? {
                items.push(id);
            }
        }
        let name = main.name.as_deref().unwrap_or("main");
        Ok(ctx.builder.function(name, Vec::new(), items, Loc::default()))
    }

    fn translate_specification_stmt<A: chomsky_uir::Analysis<IKun>>(
        &self,
        spec: &SpecificationStmt,
        _ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        match spec {
            SpecificationStmt::TypeDeclaration(_node) => {
                // TODO: 实现变量声明
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn translate_executable_stmt<A: chomsky_uir::Analysis<IKun>>(
        &self,
        exec: &ExecutableStmt,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::default();
        match exec {
            ExecutableStmt::Assignment(node) => {
                let var = self.translate_expr(&node.variable, ctx)?;
                let expr = self.translate_expr(&node.expression, ctx)?;
                Ok(ctx.builder.extension("=", vec![var, expr], loc))
            }
            ExecutableStmt::Call(node) => {
                let args = node.arguments.iter()
                    .map(|arg| self.translate_expr(arg, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                let callee = ctx.builder.symbol(&node.procedure_name, loc);
                Ok(ctx.builder.call(callee, args, loc))
            }
            ExecutableStmt::Print(node) => {
                let args = node.output_items.iter()
                    .map(|arg| self.translate_expr(arg, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ctx.builder.cross_lang_call("nyar", "io", "println", args, loc))
            }
            ExecutableStmt::IfConstruct(node) => {
                let cond = self.translate_expr(&node.condition, ctx)?;
                let then_body = node.then_part.iter()
                    .map(|s| self.translate_executable_stmt(s, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                let then_id = ctx.builder.seq(then_body, loc);
                
                let mut current_else = if let Some(else_part) = &node.else_part {
                    let else_body = else_part.iter()
                        .map(|s| self.translate_executable_stmt(s, ctx))
                        .collect::<Result<Vec<_>, _>>()?;
                    ctx.builder.seq(else_body, loc)
                } else {
                    ctx.builder.seq(vec![], loc)
                };

                for (else_cond, else_body) in node.else_if_parts.iter().rev() {
                    let cond_id = self.translate_expr(else_cond, ctx)?;
                    let body_ids = else_body.iter()
                        .map(|s| self.translate_executable_stmt(s, ctx))
                        .collect::<Result<Vec<_>, _>>()?;
                    let body_id = ctx.builder.seq(body_ids, loc);
                    current_else = ctx.builder.branch(cond_id, body_id, current_else, loc);
                }

                Ok(ctx.builder.branch(cond, then_id, current_else, loc))
            }
            ExecutableStmt::DoConstruct(node) => {
                if let Some(DoControl::Iterative { variable, start, end, step }) = &node.control {
                    let start_id = self.translate_expr(start, ctx)?;
                    let end_id = self.translate_expr(end, ctx)?;
                    let step_id = if let Some(s) = step {
                        self.translate_expr(s, ctx)?
                    } else {
                        ctx.builder.int(1, loc)
                    };
                    
                    let body = node.body.iter()
                        .map(|s| self.translate_executable_stmt(s, ctx))
                        .collect::<Result<Vec<_>, _>>()?;
                    let body_id = ctx.builder.seq(body, loc);
                    let var_id = ctx.builder.symbol(variable, loc);
                    
                    // 这里简化为 extension 调用，实际可能需要更复杂的循环结构
                    Ok(ctx.builder.extension("do_loop", vec![
                        var_id,
                        start_id, end_id, step_id, body_id
                    ], loc))
                } else {
                    // TODO: 其他类型的循环
                    Ok(ctx.builder.seq(vec![], loc))
                }
            }
            _ => Ok(ctx.builder.seq(vec![], loc)),
        }
    }

    fn translate_expr<A: chomsky_uir::Analysis<IKun>>(
        &self,
        expr: &ExprNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let loc = Loc::default();
        match expr {
            ExprNode::Literal(lit) => match lit {
                Literal::Integer(v) => Ok(ctx.builder.int(v.parse().unwrap_or(0), loc)),
                Literal::Real(v) => Ok(ctx.builder.float(v.parse().unwrap_or(0.0), loc)),
                Literal::Character(v) => Ok(ctx.builder.string(v, loc)),
                Literal::Logical(v) => Ok(ctx.builder.bool(*v, loc)),
                Literal::Complex(r, i) => {
                    let rv = ctx.builder.float(r.parse().unwrap_or(0.0), loc);
                    let iv = ctx.builder.float(i.parse().unwrap_or(0.0), loc);
                    Ok(ctx.builder.extension("complex", vec![rv, iv], loc))
                }
            },
            ExprNode::Variable(var) => {
                let mut current = ctx.builder.symbol(&var.name, loc);
                for selector in &var.selectors {
                    match selector {
                        VariableSelector::ArraySubscript(indices) => {
                            let mut args = vec![current];
                            for idx in indices {
                                args.push(self.translate_expr(idx, ctx)?);
                            }
                            current = ctx.builder.extension("index", args, loc);
                        }
                        VariableSelector::Component(name) => {
                            let member_id = ctx.builder.symbol(name, loc);
                            current = ctx.builder.extension(".", vec![current, member_id], loc);
                        }
                        _ => {}
                    }
                }
                Ok(current)
            }
            ExprNode::Unary(op, expr) => {
                let e = self.translate_expr(expr, ctx)?;
                let op_str = match op {
                    UnaryOp::Plus => "+",
                    UnaryOp::Minus => "-",
                    UnaryOp::Not => "!",
                };
                Ok(ctx.builder.extension(op_str, vec![e], loc))
            }
            ExprNode::Binary(lhs, op, rhs) => {
                let l = self.translate_expr(lhs, ctx)?;
                let r = self.translate_expr(rhs, ctx)?;
                let op_str = match op {
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    BinaryOp::Pow => "**",
                    BinaryOp::Eq => "==",
                    BinaryOp::Ne => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::And => "&&",
                    BinaryOp::Or => "||",
                    _ => "unknown",
                };
                Ok(ctx.builder.extension(op_str, vec![l, r], loc))
            }
            ExprNode::Call(name, args) => {
                let arg_ids = args.iter()
                    .map(|arg| self.translate_expr(arg, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                let callee = ctx.builder.symbol(name, loc);
                Ok(ctx.builder.call(callee, arg_ids, loc))
            }
            ExprNode::Paren(expr) => self.translate_expr(expr, ctx),
            _ => Ok(ctx.builder.seq(vec![], loc)),
        }
    }

    fn translate_subroutine<A: chomsky_uir::Analysis<IKun>>(
        &self,
        sub: &SubroutineNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        // 翻译 specification_part
        for spec in &sub.specification_part {
            if let Some(id) = self.translate_specification_stmt(spec, ctx)? {
                items.push(id);
            }
        }
        // 翻译 execution_part
        for exec in &sub.execution_part {
            items.push(self.translate_executable_stmt(exec, ctx)?);
        }
        // 翻译 internal_subprograms
        for internal in &sub.internal_subprograms {
            if let Some(id) = self.translate_program_unit(internal, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.function(&sub.name, sub.parameters.clone(), items, Loc::default()))
    }

    fn translate_function<A: chomsky_uir::Analysis<IKun>>(
        &self,
        func: &FunctionNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        // 翻译 specification_part
        for spec in &func.specification_part {
            if let Some(id) = self.translate_specification_stmt(spec, ctx)? {
                items.push(id);
            }
        }
        // 翻译 execution_part
        for exec in &func.execution_part {
            items.push(self.translate_executable_stmt(exec, ctx)?);
        }
        // 翻译 internal_subprograms
        for internal in &func.internal_subprograms {
            if let Some(id) = self.translate_program_unit(internal, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.function(&func.name, func.parameters.clone(), items, Loc::default()))
    }

    fn translate_module<A: chomsky_uir::Analysis<IKun>>(
        &self,
        module: &ModuleNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        // 翻译 specification_part
        for spec in &module.specification_part {
            if let Some(id) = self.translate_specification_stmt(spec, ctx)? {
                items.push(id);
            }
        }
        // 翻译 module_subprograms
        for sub in &module.module_subprograms {
            if let Some(id) = self.translate_program_unit(sub, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.module(&module.name, items, Loc::default()))
    }

    fn translate_submodule<A: chomsky_uir::Analysis<IKun>>(
        &self,
        submodule: &SubmoduleNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let items = Vec::new();
        // TODO
        Ok(ctx.builder.module(&submodule.name, items, Loc::default()))
    }

    fn translate_block_data<A: chomsky_uir::Analysis<IKun>>(
        &self,
        block_data: &BlockDataNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let items = Vec::new();
        // TODO
        let name = block_data.name.as_deref().unwrap_or("block_data");
        Ok(ctx.builder.module(name, items, Loc::default()))
    }
}
