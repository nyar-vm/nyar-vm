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
        let items = Vec::new();
        // TODO: 翻译 specification_part, execution_part, internal_subprograms
        let name = main.name.as_deref().unwrap_or("main");
        Ok(ctx.builder.module(name, items, Loc::default()))
    }

    fn translate_subroutine<A: chomsky_uir::Analysis<IKun>>(
        &self,
        sub: &SubroutineNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let items = Vec::new();
        // TODO
        Ok(ctx.builder.module(&sub.name, items, Loc::default()))
    }

    fn translate_function<A: chomsky_uir::Analysis<IKun>>(
        &self,
        func: &FunctionNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let items = Vec::new();
        // TODO
        Ok(ctx.builder.module(&func.name, items, Loc::default()))
    }

    fn translate_module<A: chomsky_uir::Analysis<IKun>>(
        &self,
        module: &ModuleNode,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let items = Vec::new();
        // TODO
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
