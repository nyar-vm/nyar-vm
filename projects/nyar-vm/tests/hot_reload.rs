use nyar_vm::bytecode::format::{Constant, NyarcModule, minimal_module_with_chunk};
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::bytecode::decoder::Instruction;

#[test]
fn test_hot_reload_basic() {
    let mut vm = NyarVM::new(vec![], vec![], vec![], vec![], vec![], vec![]);

    // Phase 1: Load initial module with a "version" function returning 1
    let module_v1 = minimal_module_with_chunk(
        vec![Instruction::I32Const(1).encode()[0], 0, 0, 0, Instruction::Return.encode()[0]], 
        vec![]
    );
    // Note: This manual encoding is just for demo. In reality, we use a proper assembler.
    // For this test, let's use a simpler approach.
    
    vm.load_module(module_v1);
    assert_eq!(vm.chunks.len(), 1);
    
    // Phase 2: "Hot-reload" by adding a new module with a different version
    let module_v2 = minimal_module_with_chunk(
        vec![Instruction::I32Const(2).encode()[0], 0, 0, 0, Instruction::Return.encode()[0]], 
        vec![]
    );
    vm.load_module(module_v2);
    assert_eq!(vm.chunks.len(), 2);
    
    // The VM now has both versions of the code. 
    // New calls can be directed to the new chunk index.
}
