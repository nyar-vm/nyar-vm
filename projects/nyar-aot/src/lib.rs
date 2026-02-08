use chomsky::optimizer::UniversalOptimizer;
use chomsky_extract::{Backend, BackendArtifact, IKunTree};
use chomsky_uir::IKun;
// use chomsky_cost::CostModel;
use gaia_types::helpers::Architecture;
use nyar_types::VmError;

pub struct NyarAot<A: chomsky_uir::egraph::Analysis<IKun> + 'static = ()>
where
    A::Data: chomsky_uir::egraph::HasDebugInfo,
{
    pub optimizer: UniversalOptimizer<A>,
}

impl<A: chomsky_uir::egraph::Analysis<IKun> + 'static> NyarAot<A>
where
    A::Data: chomsky_uir::egraph::HasDebugInfo,
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
    ) -> Result<BackendArtifact, VmError> {
        let id = self.add_intent(ikun);
        self.saturate();
        let tree = self.extract(id, backend.get_model());

        let artifact = backend
            .generate(&tree)
            .map_err(|e| {
                VmError::new(
                    0x2001,
                    nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::RuntimeError(format!("Backend error: {:?}", e))),
                    Default::default(),
                )
            })?;

        Ok(artifact)
    }

    /// Adds an intent to the internal EGraph.
    pub fn add_intent(&mut self, ikun: &IKun) -> chomsky_uir::egraph::Id {
        self.optimizer.add_intent(ikun)
    }

    /// Runs saturation search on the internal EGraph.
    pub fn saturate(&mut self) {
        self.optimizer.saturate();
    }

    /// Extracts the best candidate from the internal EGraph using a cost model.
    pub fn extract(
        &self,
        root_id: chomsky_uir::egraph::Id,
        cost_model: &dyn chomsky::cost::CostModel,
    ) -> chomsky_uir::IKunTree {
        self.optimizer.extract(root_id, cost_model)
    }
}

impl<A: chomsky_uir::egraph::Analysis<IKun> + 'static + Default> Default for NyarAot<A>
where
    A::Data: chomsky_uir::egraph::HasDebugInfo,
{
    fn default() -> Self {
        Self::new()
    }
}
