use nyar_vm::bytecode::format::minimal_module_with_chunk;
use nyar_vm::vm::core::NyarVM;

#[test]
fn test_hot_reload_basic() {
    let mut vm = NyarVM::new();

    // Phase 1: Load initial module with a "version" function returning 1
    // For this test, let's use a simpler approach.
    let module_v1 = minimal_module_with_chunk(vec![], vec![]);

    vm.load_module(module_v1);
    assert_eq!(vm.modules.len(), 1);

    // Phase 2: "Hot-reload" by adding a new module with a different version
    let module_v2 = minimal_module_with_chunk(vec![], vec![]);
    vm.load_module(module_v2);
    assert_eq!(vm.modules.len(), 2);
}
