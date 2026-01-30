use chomsky::optimizer::UniversalOptimizer;
use chomsky_extract::{Backend, BackendArtifact};
use chomsky_uir::IKun;
use nyar_types::VmError;

pub struct NyarAot {
    optimizer: UniversalOptimizer<()>,
}

impl NyarAot {
    pub fn new() -> Self {
        Self {
            optimizer: UniversalOptimizer::new(),
        }
    }

    /// Compiles an IKun intent to a BackendArtifact.
    pub fn compile(&mut self, ikun: &IKun, backend: &dyn Backend) -> Result<BackendArtifact, VmError> {
        // 1. Add intent to optimizer (EGraph)
        let id = self.optimizer.add_intent(ikun);
        
        // 2. Saturate (Optimize)
        self.optimizer.saturate();
        
        // 3. Extract best IKunTree
        let tree = self.optimizer.extract(id, backend.get_model());
        
        // 4. Generate artifact using backend
        let artifact = backend.generate(&tree)
            .map_err(|e| VmError::RuntimeError(format!("Backend error: {:?}", e)))?;
            
        Ok(artifact)
    }
}

impl Default for NyarAot {
    fn default() -> Self {
        Self::new()
    }
}
