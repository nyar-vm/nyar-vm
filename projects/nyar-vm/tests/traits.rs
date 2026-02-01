use nyar_types::QualifiedName;
use nyar_vm::bytecode::format::{Chunk, Constant, ExportInfo, ImplInfo, NyarcModule};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::vm::core::NyarVM;
use std::sync::atomic::AtomicU32;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_witness_table_instructions() {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Chunk 0: The main program
    // GetWitnessTable(1, 2)
    // WitnessMethod(0)
    // CallClosure(0)
    // Return
    let mut main_code = Vec::new();
    main_code.push(Opcode::GetWitnessTable as u8);
    main_code.extend_from_slice(&1u16.to_le_bytes());
    main_code.extend_from_slice(&2u16.to_le_bytes());
    main_code.push(Opcode::WitnessMethod as u8);
    main_code.extend_from_slice(&0u16.to_le_bytes());
    main_code.push(Opcode::CallClosure as u8);
    main_code.push(0); // 0 args
    main_code.push(Opcode::Return as u8);

    // Chunk 1: The method implementation
    // Push(const 0) -> 42
    // Return
    let mut method_code = Vec::new();
    method_code.push(Opcode::Push as u8);
    method_code.extend_from_slice(&0u16.to_le_bytes());
    method_code.push(Opcode::Return as u8);

    let module = NyarcModule {
        version: 1,
        flags: 0,
        timestamp: ts,
        constants: vec![Constant::Int(42)],
        effects: vec![],
        chunks: vec![
            Chunk {
                locals: 0,
                upvalues: 0,
                max_stack: 8,
                code: main_code,
                handlers: vec![],
                lines: vec![],
                decoded: None,
                hotness: AtomicU32::new(0),
            },
            Chunk {
                locals: 0,
                upvalues: 0,
                max_stack: 8,
                code: method_code,
                handlers: vec![],
                lines: vec![],
                decoded: None,
                hotness: AtomicU32::new(0),
            },
        ],
        classes: vec![],
        traits: vec![],
        impls: vec![ImplInfo {
            class_idx: 1,
            trait_idx: 2,
            methods: vec![1], // chunk 1 is the method
        }],
        imports: vec![],
        exports: vec![ExportInfo {
            symbol: QualifiedName::new(vec!["main".to_string()]),
            chunk_idx: 0,
        }],
    };

    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_int(), 42);
}
