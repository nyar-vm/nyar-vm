use nyar_types::CliError;
use nyar_vm::bytecode::format::Chunk;
use nyar_vm::vm::NyarVM;
use std::io::Write;

pub fn bench() -> Result<(), CliError> {
    let mut code = Vec::new();
    code.push(nyar_vm::bytecode::opcode::Opcode::Push as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(nyar_vm::bytecode::opcode::Opcode::Return as u8);
    let chunk = Chunk {
        locals: 0,
        upvalues: 0,
        max_stack: 8,
        code,
        ..Default::default()
    };
    let module = nyar_vm::bytecode::format::NyarcModule {
        constants: vec![nyar_vm::bytecode::format::Constant::Int(1)],
        chunks: vec![chunk],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let mut acc = 0i64;
    for _ in 0..10000 {
        let _ = vm.execute(module_idx, 0);
        acc += 1;
    }
    std::io::stdout().write_all(format!("{}\n", acc).as_bytes())?;
    Ok(())
}
