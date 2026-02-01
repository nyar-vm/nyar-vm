use nyar_vm::bytecode::format::NyarModule;
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::NyarVM;
use nyar_vm::vm::VmError;

#[test]
fn test_halt_instruction() {
    let mut code = Vec::new();
    code.push(Opcode::Halt as u8);
    
    let module = NyarModule {
        constants: vec![],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            max_stack: 8,
            code,
            ..Default::default()
        }],
        ..Default::default()
    };
    
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let err = vm.execute(module_idx, 0).err().expect("Halt should return an error");
    
    match err {
        VmError::RuntimeError(msg) => {
            assert!(msg.contains("Halt instruction encountered"));
        }
        _ => panic!("Expected RuntimeError, got {:?}", err),
    }
}
