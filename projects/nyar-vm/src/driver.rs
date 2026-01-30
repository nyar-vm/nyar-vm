use crate::bytecode::compiler::NyarBackend;
use crate::vm::interpreter::NyarVM;
use nyar_types::{NyarError, NyarFrontend};

pub struct NyarDriver;

impl NyarDriver {
    pub fn run_source<F: NyarFrontend>(frontend: &F, source: &str) -> Result<(), NyarError> {
        let ast = frontend.parse(source)?;
        let tree = frontend.lower(&ast)?;
        let mut backend = NyarBackend::new();
        backend.lower_tree(&tree)?;
        let module = backend.finish();
        let mut vm = NyarVM::new();
        let module_idx = vm.load_module(module);
        vm.execute(module_idx, 0).map(|_| ()).map_err(NyarError::from)
    }

    pub fn compile_to_native<F: NyarFrontend>(
        _frontend: &F,
        _source: &str,
        _output: &str,
    ) -> Result<(), NyarError> {
        // Implementation for AOT compilation to native
        todo!("AOT compilation to native is not yet implemented")
    }

    pub fn compile_to_wasm<F: NyarFrontend>(
        _frontend: &F,
        _source: &str,
        _output: &str,
    ) -> Result<(), NyarError> {
        // Implementation for AOT compilation to WASM
        todo!("AOT compilation to WASM is not yet implemented")
    }
}
