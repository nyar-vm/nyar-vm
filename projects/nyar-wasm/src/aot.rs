use nyar_error::WasmAotError;
use nyar_vm::bytecode::format::NyarcModule;

pub fn compile_module_to_wasm(module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
    nyar_vm::aot::compile_module_to_wasm(module)
}
