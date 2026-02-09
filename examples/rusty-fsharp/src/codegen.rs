//! FSharp 到 Nyar 字节码的翻译器

use chomsky_types::Loc;
use chomsky_uir::{ConstraintAnalysis, EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::{NyarError};
use oak_fsharp::ast::*;

/// FSharp 翻译器上下文
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
        ast: &FSharpRoot,
        egraph: &mut EGraph<IKun, ConstraintAnalysis>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut ctx = TranslatorContext::new(egraph);
        self.translate_root(ast, &mut ctx)
    }

    pub fn translate_to_tree(&self, ast: &FSharpRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::<IKun, ConstraintAnalysis>::new();
        let root_id = self.translate_to_graph(ast, &mut egraph)?;

        let extractor =
            chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());

        Ok(extractor.extract(root_id))
    }

    pub fn translate_root<A: chomsky_uir::Analysis<IKun>>(
        &self,
        root: &FSharpRoot,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &root.items {
            if let Some(id) = self.translate_item(item, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.seq(items, Loc::default()))
    }

    pub fn translate_item<A: chomsky_uir::Analysis<IKun>>(
        &self,
        item: &Item,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<Option<chomsky_uir::egraph::Id>, NyarError> {
        match item {
            Item::Namespace(ns) => Ok(Some(self.translate_namespace(ns, ctx)?)),
            Item::Module(m) => Ok(Some(self.translate_module(m, ctx)?)),
            Item::Open(_o) => Ok(None), // Open directives are handled by resolver usually
            Item::Binding(b) => Ok(Some(self.translate_binding(b, ctx)?)),
        }
    }

    fn translate_namespace<A: chomsky_uir::Analysis<IKun>>(
        &self,
        ns: &NamespaceDeclaration,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &ns.items {
            if let Some(id) = self.translate_item(item, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.module(&ns.name, items))
    }

    fn translate_module<A: chomsky_uir::Analysis<IKun>>(
        &self,
        m: &ModuleDeclaration,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        let mut items = Vec::new();
        for item in &m.items {
            if let Some(id) = self.translate_item(item, ctx)? {
                items.push(id);
            }
        }
        Ok(ctx.builder.module(&m.name, items))
    }

    fn translate_binding<A: chomsky_uir::Analysis<IKun>>(
        &self,
        _b: &Binding,
        ctx: &mut TranslatorContext<'_, A>,
    ) -> Result<chomsky_uir::egraph::Id, NyarError> {
        // Placeholder for let binding
        Ok(ctx.builder.seq(vec![], Loc::default()))
    }
}
