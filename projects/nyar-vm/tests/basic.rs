use nyar_vm::bytecode::format::{NyarcModule, Constant, minimal_module_with_chunk};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::vm::interpreter::VM;
use nyar_vm::vm::VmError;

#[test]
fn run_push_const_return() {
    let mut code = Vec::new();
    code.push(Opcode::PushConst as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![Constant::Int(42)]);
    let data = module.encode();
    let parsed = NyarcModule::parse(&data).unwrap();
    let chunk = parsed.chunks[0].clone();
    let mut vm = VM::new(parsed.constants, parsed.effects);
    let v = vm.execute(&chunk).unwrap();
    unsafe { assert_eq!(v.as_int(), 42); }
}

#[test]
fn perform_throw_unhandled() {
    let mut code = Vec::new();
    code.push(Opcode::PushConst as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::Perform as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(1u8);
    code.push(Opcode::Return as u8);
    let module = NyarcModule { version: 1, flags: 0, timestamp: 0, constants: vec![Constant::Int(7)], effects: vec!["throw".to_string()], chunks: vec![nyar_vm::bytecode::format::Chunk { locals: 0, upvalues: 0, max_stack: 8, code, handlers: vec![] }] };
    let mut vm = VM::new(module.constants.clone(), module.effects.clone());
    let chunk = module.chunks[0].clone();
    let err = vm.execute(&chunk).err().unwrap();
    match err { VmError::UnhandledError => {} _ => panic!() }
}

