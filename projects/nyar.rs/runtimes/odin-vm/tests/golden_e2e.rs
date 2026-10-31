//! End-to-end golden path: encode `.nyar` → load → execute.

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinLayout, OdinModuleData, encode_module,
};
use odin_vm::{OdinVm, Value};

#[test]
fn golden_const_add_return_roundtrip() {
    let module = OdinModuleData {
        version: Odin_VERSION,
        name: "golden".to_string(),
        constants: vec![OdinConstant::Integer32(0), OdinConstant::Integer32(1)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: 12 }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: vec![
            OdinHeadCode::Constant as u8,
            0x00,
            0x00,
            0x00,
            0x00,
            OdinHeadCode::Constant as u8,
            0x01,
            0x00,
            0x00,
            0x00,
            OdinHeadCode::I32Add as u8,
            OdinHeadCode::Return as u8,
        ],
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };

    let bytes = encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load golden module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("run main");
    assert_eq!(result, Value::I32(1));
}

#[test]
fn golden_object_new_field_get_set_roundtrip() {
    // ObjectNew(0); Const(0); FieldSet(0); FieldGet(0); Return
    // layout[0].field_count = 1；常量 0 = i32 42
    let mut code = Vec::new();
    code.push(OdinHeadCode::ObjectNew as u8);
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(OdinHeadCode::Constant as u8);
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(OdinHeadCode::FieldSet as u8);
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(OdinHeadCode::FieldGet as u8);
    code.extend_from_slice(&0i32.to_le_bytes());
    code.push(OdinHeadCode::Return as u8);

    let module = OdinModuleData {
        version: Odin_VERSION,
        name: "golden-layout".to_string(),
        constants: vec![OdinConstant::Integer32(42)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: vec![OdinLayout { field_count: 1 }],
    };

    let bytes = encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load layout module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("run main");
    assert_eq!(result, Value::I32(42));
}
