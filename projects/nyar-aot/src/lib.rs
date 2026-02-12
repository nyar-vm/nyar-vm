#![warn(missing_docs)]

use chomsky::optimizer::UniversalOptimizer;
use chomsky_extract::{Backend, BackendArtifact};
use chomsky_uir::egraph::{Analysis, HasDebugInfo};
use chomsky_uir::{EGraph, IKun, IKunTree, IntentBuilder};
use nyar_types::{Id, NyarError};
use oak_core::Language;
use oak_vfs::Vfs;

pub use nyar_types::NyarContext as NyarContextBase;

/// AOT 编译器上下文
pub struct NyarContext<'a, V: Vfs, A: Analysis<IKun>> {
    /// EGraph 实例
    pub egraph: &'a mut EGraph<IKun, A>,
    /// 虚拟文件系统
    pub vfs: &'a V,
    /// 作用域层级
    pub scope: usize,
}

impl<'a, V: Vfs, A: Analysis<IKun>> NyarContext<'a, V, A> {
    pub fn new(egraph: &'a mut EGraph<IKun, A>, vfs: &'a V, scope: usize) -> Self {
        Self { egraph, vfs, scope }
    }

    pub fn builder(&mut self) -> IntentBuilder<'_, A> {
        IntentBuilder::new(&mut self.egraph)
    }
}

pub use nyar_types::NyarFrontend as NyarFrontendBase;

/// Nyar 前端接口 trait
/// 所有语言前端必须实现此接口，以便接入 Nyar 编译体系
pub trait NyarFrontend<A: Analysis<IKun> = ()>: Default + NyarFrontendBase<A>
where
    A::Data: HasDebugInfo,
{
    /// 绑定的语言类型
    type Language: Language;

    /// 解析源代码为 AST
    fn parse(&self, source: &str) -> Result<Self::Language::TypedRoot, NyarError>;

    /// 将 AST 转换为 IR 并注入 EGraph
    /// 统一的接入接口，支持 EGraph 优化流
    fn lower_unified<V: Vfs>(&self, ast: &Self::Language::TypedRoot, ctx: &mut NyarContext<'_, V, A>) -> Id;

    /// 默认实现：利用 lower_unified 生成 IKunTree
    fn lower<V: Vfs>(&self, ast: &Self::Language::TypedRoot, vfs: &V) -> Result<IKunTree, NyarError>
    where
        A::Data: HasDebugInfo,
    {
        let mut egraph = EGraph::new();
        let mut ctx = NyarContext::new(&mut egraph, vfs, 0);
        let root_id = self.lower_unified(ast, &mut ctx);

        // 此处可以插入统一的优化流程
        egraph.rebuild();

        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        Ok(extractor.extract(root_id))
    }

    /// 利用 Gaia 编译到特定目标
    fn compile_to_gaia<V: Vfs>(
        &self,
        ast: &Self::Language::TypedRoot,
        vfs: &V,
        target: &str,
    ) -> Result<chomsky_extract::BackendArtifact, NyarError>
    where
        A::Data: HasDebugInfo,
    {
        let tree = self.lower(ast, vfs)?;
        // 注意：此处假设 chomsky_emit 提供了 GaiaEmitter
        // 实际使用时需要引入 chomsky-emit
        let emitter = chomsky_emit::GaiaEmitter::new(target).standalone();
        emitter.generate(&tree).map_err(|e| NyarError::Compile(format!("Gaia error: {:?}", e)))
    }
}

impl<A: Analysis<IKun>> NyarFrontendBase<A> for dyn NyarFrontend<A, Language = ()>
where
    A::Data: HasDebugInfo,
{
    type Language = ();
}

pub struct NyarAot<A: Analysis<IKun> + 'static = ()>
where
    A::Data: HasDebugInfo,
{
    pub optimizer: UniversalOptimizer<A>,
}

impl<A: Analysis<IKun> + 'static> NyarAot<A>
where
    A::Data: HasDebugInfo,
{
    pub fn new() -> Self
    where
        A: Default,
    {
        Self {
            optimizer: UniversalOptimizer::new(),
        }
    }

    /// Compiles an IKun intent to a BackendArtifact.
    pub fn compile(
        &mut self,
        ikun: &IKun,
        backend: &dyn Backend,
    ) -> Result<BackendArtifact, NyarError> {
        let id = self.add_intent(ikun);
        self.saturate();
        let tree = self.extract(id, backend.get_model());

        let artifact = backend
            .generate(&tree)
            .map_err(|e| {
                NyarError::Compile(format!("Backend error: {:?}", e))
            })?;

        Ok(artifact)
    }

    /// Adds an intent to the internal EGraph.
    pub fn add_intent(&mut self, ikun: &IKun) -> Id {
        self.optimizer.add_intent(ikun)
    }

    /// Runs saturation search on the internal EGraph.
    pub fn saturate(&mut self) {
        self.optimizer.saturate();
    }

    /// Extracts the best candidate from the internal EGraph using a cost model.
    pub fn extract(
        &self,
        root_id: Id,
        cost_model: &dyn chomsky::cost::CostModel,
    ) -> chomsky_uir::IKunTree {
        self.optimizer.extract(root_id, cost_model)
    }
}

impl<A: Analysis<IKun> + 'static + Default> Default for NyarAot<A>
where
    A::Data: HasDebugInfo,
{
    fn default() -> Self {
        Self::new()
    }
}
