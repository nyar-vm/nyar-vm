use nyar_vm::bytecode::compiler::NyarBackend;
use nyar_vm::vm::core::NyarVM;
use chomsky_extract::IKunTree;
use nyar_vm::bytecode::format::Chunk;
use nyar_vm::bytecode::opcode::Opcode;

#[test]
fn test_class_compilation() {
    let mut backend = NyarBackend::new();
    
    // 1. Define a class "Point" with fields "x" and "y"
    let class_def = IKunTree::Extension(
        "class".to_string(),
        vec![
            IKunTree::StringConstant("Point".to_string()),
            IKunTree::Seq(vec![
                IKunTree::Extension("field".to_string(), vec![IKunTree::StringConstant("x".to_string())]),
                IKunTree::Extension("field".to_string(), vec![IKunTree::StringConstant("y".to_string())]),
            ]),
        ],
    );
    
    // 2. Instantiate "Point" with values 10 and 20
    let instantiate = IKunTree::Extension(
        "new".to_string(),
        vec![
            IKunTree::StringConstant("Point".to_string()),
            IKunTree::Seq(vec![
                IKunTree::Constant(10),
                IKunTree::Constant(20),
            ]),
        ],
    );
    
    // Register class
    backend.lower_tree(&class_def).unwrap();
    
    // Compile instantiation code
    let mut code = backend.lower_tree(&instantiate).unwrap();
    if code.last() != Some(&(Opcode::Return as u8)) {
        code.push(Opcode::Return as u8);
    }
    
    let mut module = backend.finish();
    
    // Add a chunk with the instantiation code
    module.chunks.push(Chunk {
        locals: 32,
        upvalues: 0,
        max_stack: 64,
        code,
        ..Default::default()
    });
    
    // Verify class registration
    assert_eq!(module.classes.len(), 1);
    assert_eq!(module.classes[0].name.to_string(), "Point");
    assert_eq!(module.classes[0].fields, vec!["x", "y"]);
    
    // Verify bytecode execution
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    
    // Execute the chunk we just added
    let result = vm.execute(module_idx, 0).unwrap();
    
    // Result should be an object. 
    // We can't easily check the fields without more imports, but we can check it's not null.
    assert!(!result.is_null());
}

#[test]
fn test_invalid_class_index_does_not_panic() {
    let mut module = nyar_vm::bytecode::format::NyarModule::default();
    
    // Create code with NewObject instruction using index 99 (invalid)
    let mut code = Vec::new();
    code.push(Opcode::NewObject as u8);
    code.extend_from_slice(&99u16.to_le_bytes());
    code.push(Opcode::Return as u8);
    
    module.chunks.push(Chunk {
        locals: 32,
        upvalues: 0,
        max_stack: 64,
        code,
        ..Default::default()
    });
    
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    
    // This should not panic, but return an error
    let result = vm.execute(module_idx, 0);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(*err.kind, nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::IndexOutOfBounds(99))));
}
