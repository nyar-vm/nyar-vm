use nyar_vm::NyarVM;
use nyar_vm::vm::value::Value;
use nyar_vm::bytecode::format::{NyarcModule, Constant};

#[test]
fn test_ffi_signature_validation() {
    let mut vm = NyarVM::new();
    
    // Create a dummy module with "native_add" constant
    let mut module = NyarcModule::default();
    module.constants.push(Constant::String("native_add".to_string()));
    let module_idx = vm.load_module(module);
    
    // Set up a frame
    let frame = nyar_vm::vm::value::Frame {
        instrs: std::sync::Arc::new(vec![]),
        ip: 0,
        locals: vec![Value::null(); 32],
        upvalues: vec![None; 32],
        closure: Value::null(),
        module_idx,
        chunk_idx: None,
    };
    vm.frames.push(frame);
    
    // 1. Valid call
    vm.push(Value::int(10)).unwrap();
    vm.push(Value::int(20)).unwrap();
    let res = vm.execute_ffi_call(0, 2, module_idx);
    assert!(res.is_ok());
    assert_eq!(vm.pop().unwrap().as_int(), 30);
    
    // 2. Invalid argument count
    vm.push(Value::int(10)).unwrap();
    let res = vm.execute_ffi_call(0, 1, module_idx);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(format!("{:?}", e).contains("expects 2 arguments, got 1"));
    }
    
    // 3. Invalid argument type
    vm.push(Value::int(10)).unwrap();
    vm.push(Value::float(20.0)).unwrap();
    let res = vm.execute_ffi_call(0, 2, module_idx);
    assert!(res.is_err());
    if let Err(e) = res {
        assert!(format!("{:?}", e).contains("argument 1 type mismatch"));
    }
}
