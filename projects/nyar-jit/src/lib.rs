use chomsky_full::optimizer::UniversalOptimizer;
use chomsky_full::extract::{Backend, BackendArtifact, IKunTree};
use chomsky_uir::{IKun, Id};
use gaia_jit::JitMemory;
use nyar_error::VmError;

pub struct NyarJit {
    optimizer: UniversalOptimizer<()>,
    jit_mem: JitMemory,
}

impl NyarJit {
    pub fn new(capacity: usize) -> Result<Self, VmError> {
        let optimizer = UniversalOptimizer::new();
        let jit_mem = JitMemory::new(capacity).map_err(|e| VmError::RuntimeError(e.to_string()))?;
        Ok(Self { optimizer, jit_mem })
    }

    /// Compiles and executes an IKun intent.
    pub fn execute(&mut self, ikun: &IKun, backend: &dyn Backend) -> Result<(), VmError> {
        // 1. Add intent to optimizer (EGraph)
        let id = self.optimizer.add_intent(ikun);
        
        // 2. Saturate (Optimize)
        self.optimizer.saturate();
        
        // 3. Extract best IKunTree
        let tree = self.optimizer.extract(id, backend.get_model());
        
        // 4. Generate machine code using backend
        let artifact = backend.generate(&tree)
            .map_err(|e| VmError::RuntimeError(format!("Backend error: {:?}", e)))?;
            
        match artifact {
            BackendArtifact::Binary(code) => {
                // 5. Write to JIT memory
                self.jit_mem.write(&code).map_err(|e| VmError::RuntimeError(e.to_string()))?;
                
                // 6. Make executable and run
                let ptr = self.jit_mem.make_executable().map_err(|e| VmError::RuntimeError(e.to_string()))?;
                
                unsafe {
                    let f: unsafe extern "C" fn() = std::mem::transmute(ptr);
                    f();
                }
            }
            BackendArtifact::Source(_) => {
                return Err(VmError::RuntimeError("JIT backend must produce binary, not source code".to_string()));
            }
        }
        
        Ok(())
    }
}
