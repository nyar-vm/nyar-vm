use nyar_error::CliError;
use nyar_vm::bytecode::decoder::Decoder;
use nyar_vm::bytecode::format::Chunk;
use nyar_vm::vm::interpreter::NyarVM;
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
        handlers: vec![],
    };
    let program = Decoder::new(&chunk.code).decode_all().unwrap_or_default();
    let consts = vec![nyar_vm::bytecode::format::Constant::Int(1)];
    let mut vm = NyarVM::new(consts, vec![chunk], vec![], vec![], vec![], vec![]);
    let mut acc = 0i64;
    for _ in 0..10000 {
        let _ = vm.execute(&program);
        acc += 1;
    }
    std::io::stdout().write_all(format!("{}\n", acc).as_bytes())?;
    Ok(())
}
