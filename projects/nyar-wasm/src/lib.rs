pub mod aot;

pub use aot::{
    compile_module_to_wasm, compile_module_to_wasm_with, ClosureTypeSelectionMode,
    WasmCompileOptions,
};
pub use nyar_error::WasmAotError;

#[cfg(test)]
mod tests {
    use crate::compile_module_to_wasm;
    use nyar_vm::bytecode::format::{minimal_module_with_chunk, Constant};
    use nyar_vm::bytecode::opcode::Opcode;

    #[test]
    fn compile_push_const_return_to_wasm() {
        let mut code = Vec::new();
        code.push(Opcode::Push as u8);
        code.extend_from_slice(&0u16.to_le_bytes());
        code.push(Opcode::Return as u8);
        let module = minimal_module_with_chunk(code, vec![Constant::Int(42)]);
        let wasm = compile_module_to_wasm(&module).expect("aot compile should succeed");
        assert!(wasm.len() > 8);
        assert_eq!(&wasm[0..4], b"\0asm");
    }
}
