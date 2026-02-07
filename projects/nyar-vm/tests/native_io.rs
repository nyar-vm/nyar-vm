use nyar_vm::NyarVM;
use nyar_vm::vm::value::Value;
use nyar_vm::bytecode::format::{NyarcModule, Constant};
use nyar_types::QualifiedName;

#[test]
fn test_std_io_println_effect() {
    let mut vm = NyarVM::new();
    
    // Create a dummy module with "std::io::println" constant
    let mut module = NyarcModule::default();
    module.constants.push(Constant::QualifiedName(QualifiedName::from("std::io::println")));
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
        location: Default::default(),
    };
    vm.frames.push(frame);
    
    // Push argument
    vm.push(Value::string("Hello, Effect!".to_string(), &vm.gc)).unwrap();
    
    // Execute perform (index 0 is "std.io.println", 1 argument)
    let res = vm.execute_perform(0, 1, module_idx);
    assert!(res.is_ok());
    
    // Check if message was logged
    let logs = vm.trace_log.lock().unwrap();
    assert!(logs.contains(&"Hello, Effect!".to_string()));
}

#[test]
fn test_std_fs_write_read_effect() {
    let mut vm = NyarVM::new();
    let temp_file = "test_effect.txt";
    
    // Create a dummy module with FS constants
    let mut module = NyarcModule::default();
    module.constants.push(Constant::QualifiedName(QualifiedName::from("std::fs::write"))); // 0
    module.constants.push(Constant::QualifiedName(QualifiedName::from("std::fs::read_to_string"))); // 1
    module.constants.push(Constant::QualifiedName(QualifiedName::from("std::fs::exists"))); // 2
    module.constants.push(Constant::QualifiedName(QualifiedName::from("std::fs::remove_file"))); // 3
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
        location: Default::default(),
    };
    vm.frames.push(frame);
    
    // 1. Write file
    vm.push(Value::string(temp_file.to_string(), &vm.gc)).unwrap();
    vm.push(Value::string("Hello from FS effect!".to_string(), &vm.gc)).unwrap();
    vm.execute_perform(0, 2, module_idx).unwrap();
    
    // 2. Check exists
    vm.push(Value::string(temp_file.to_string(), &vm.gc)).unwrap();
    vm.execute_perform(2, 1, module_idx).unwrap();
    assert_eq!(vm.pop().unwrap().as_bool(), true);
    
    // 3. Read back
    vm.push(Value::string(temp_file.to_string(), &vm.gc)).unwrap();
    vm.execute_perform(1, 1, module_idx).unwrap();
    assert_eq!(vm.pop().unwrap().try_as_str().unwrap(), "Hello from FS effect!");
    
    // 4. Remove file
    vm.push(Value::string(temp_file.to_string(), &vm.gc)).unwrap();
    vm.execute_perform(3, 1, module_idx).unwrap();
    
    // 5. Check exists again
    vm.push(Value::string(temp_file.to_string(), &vm.gc)).unwrap();
    vm.execute_perform(2, 1, module_idx).unwrap();
    assert_eq!(vm.pop().unwrap().as_bool(), false);
}
