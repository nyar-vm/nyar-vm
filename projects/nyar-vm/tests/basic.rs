use nyar_vm::bytecode::format::{minimal_module_with_chunk, Constant, NyarModule};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::vm::interpreter::NyarVM;
use nyar_vm::vm::VmError;

#[test]
fn test_chunk_lines_encoding() {
    let mut code = Vec::new();
    code.push(Opcode::Return as u8);
    let lines = vec![(0, 10), (1, 20)];
    let module = NyarModule {
        constants: vec![],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            max_stack: 8,
            code: code.clone(),
            lines: lines.clone(),
            ..Default::default()
        }],
        ..Default::default()
    };
    let encoded = module.encode();
    let decoded = NyarModule::parse(&encoded).unwrap();
    let chunk = &decoded.chunks[0];
    assert_eq!(chunk.lines, lines);
}

#[test]
fn perform_throw_unhandled() {
    let mut code = Vec::new();
    code.push(Opcode::Push as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::Perform as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(1u8);
    code.push(Opcode::Return as u8);
    let module = NyarModule {
        constants: vec![Constant::Int(7)],
        effects: vec!["throw".to_string()],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            max_stack: 8,
            code,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module.clone());
    let err = vm.execute(module_idx, 0).err().unwrap();
    match err {
        VmError::UnhandledError => {}
        _ => panic!(),
    }
}

#[test]
fn run_make_tuple_and_get_element() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&2i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&3i64.to_le_bytes());
    code.push(Opcode::MakeTuple as u8);
    code.push(3u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::GetElement as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_int(), 2);
}

#[test]
fn run_has_key_tuple() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&2i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&3i64.to_le_bytes());
    code.push(Opcode::MakeTuple as u8);
    code.push(3u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::HasKey as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_bool(), true);
}

#[test]
fn tuple_set_element_same_type_should_pass() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&10i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&20i64.to_le_bytes());
    code.push(Opcode::MakeTuple as u8);
    code.push(2u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&99i64.to_le_bytes());
    code.push(Opcode::SetElement as u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::GetElement as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_int(), 99);
}

#[test]
fn tuple_set_element_mismatch_should_fail() {
    let mut code = Vec::new();
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&10i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&20i64.to_le_bytes());
    code.push(Opcode::MakeTuple as u8);
    code.push(2u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(2u8);
    code.extend_from_slice(b"hi");
    code.push(Opcode::SetElement as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let err = vm.execute(module_idx, 0).err().unwrap();
    match err {
        VmError::RuntimeError(_) => {}
        _ => panic!(),
    }
}

#[test]
fn run_has_key_object() {
    let mut code = Vec::new();
    code.push(Opcode::NewObject as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::Swap as u8);
    code.push(3u8);
    code.push(Opcode::Swap as u8);
    code.push(2u8);
    code.push(Opcode::Swap as u8);
    code.push(1u8);
    code.push(Opcode::HasKey as u8);
    code.push(Opcode::Return as u8);
    let module = NyarModule {
        version: 1,
        flags: 0,
        timestamp: 0,
        constants: vec![],
        effects: vec![],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            locals: 0,
            upvalues: 0,
            max_stack: 8,
            code,
            handlers: vec![],
            ..Default::default()
        }],
        classes: vec![nyar_vm::bytecode::format::ClassInfo {
            name: "C".to_string(),
            fields: vec!["a".to_string(), "b".to_string()],
        }],
        traits: vec![],
        impls: vec![],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module.clone());
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_bool(), true);
}

#[test]
fn run_match_variant() {
    let mut code = Vec::new();
    code.push(Opcode::NewObject as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::MatchVariant as u8);
    code.extend_from_slice(&0u16.to_le_bytes());
    code.push(Opcode::Return as u8);
    let module = NyarModule {
        constants: vec![],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            max_stack: 8,
            code,
            ..Default::default()
        }],
        classes: vec![
            nyar_vm::bytecode::format::ClassInfo {
                name: "V".to_string(),
                fields: vec![],
            },
            nyar_vm::bytecode::format::ClassInfo {
                name: "W".to_string(),
                fields: vec![],
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module.clone());
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_bool(), true);
}

#[test]
fn run_sizeof_array_string_bigint_object() {
    let psize = std::mem::size_of::<*mut ()>() as i64;
    // Array
    let mut code = Vec::new();
    code.push(Opcode::NewArray as u8);
    code.extend_from_slice(&3u16.to_le_bytes());
    code.push(Opcode::SizeOf as u8);
    code.push(Opcode::Return as u8);
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
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_int(), psize);

    // String
    let mut code2 = Vec::new();
    code2.push(Opcode::StringExt as u8);
    code2.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code2.push(3u8);
    code2.extend_from_slice(b"abc");
    code2.push(Opcode::SizeOf as u8);
    code2.push(Opcode::Return as u8);
    let module2 = minimal_module_with_chunk(code2, vec![]);
    let mut vm2 = NyarVM::new();
    let module_idx2 = vm2.load_module(module2);
    let v2 = vm2.execute(module_idx2, 0).unwrap();
    assert_eq!(v2.as_int(), psize);

    // BigInt
    let mut code3 = Vec::new();
    code3.push(Opcode::I64Ext as u8);
    code3.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code3.extend_from_slice(&42i64.to_le_bytes());
    code3.push(Opcode::BigIntExt as u8);
    code3.push(nyar_vm::bytecode::opcode::BigIntExt::FromI64 as u8);
    code3.push(Opcode::SizeOf as u8);
    code3.push(Opcode::Return as u8);
    let module3 = minimal_module_with_chunk(code3, vec![]);
    let mut vm3 = NyarVM::new();
    let module_idx3 = vm3.load_module(module3);
    let v3 = vm3.execute(module_idx3, 0).unwrap();
    assert_eq!(v3.as_int(), psize);

    // Object
    let mut code4 = Vec::new();
    code4.push(Opcode::NewObject as u8);
    code4.extend_from_slice(&0u16.to_le_bytes());
    code4.push(Opcode::SizeOf as u8);
    code4.push(Opcode::Return as u8);
    let module4 = NyarModule {
        constants: vec![],
        chunks: vec![nyar_vm::bytecode::format::Chunk {
            max_stack: 8,
            code: code4,
            ..Default::default()
        }],
        classes: vec![nyar_vm::bytecode::format::ClassInfo {
            name: "O".to_string(),
            fields: vec!["x".to_string(), "y".to_string()],
        }],
        ..Default::default()
    };
    let mut vm4 = NyarVM::new();
    let module_idx4 = vm4.load_module(module4);
    let v4 = vm4.execute(module_idx4, 0).unwrap();
    assert_eq!(v4.as_int(), psize);
}

#[test]
fn dynobject_get_set_remove_key() {
    let mut code = Vec::new();
    code.push(Opcode::NewDynObject as u8);
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&42i64.to_le_bytes());
    code.push(Opcode::SetElement as u8);
    code.push(Opcode::Dup as u8);
    code.push(0u8);
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::GetElement as u8);
    code.push(Opcode::Pop as u8);
    code.push(Opcode::Dup as u8);
    code.push(0u8);
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::RemoveKey as u8);
    code.push(Opcode::Pop as u8);
    code.push(Opcode::StringExt as u8);
    code.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    code.push(1u8);
    code.extend_from_slice(b"a");
    code.push(Opcode::HasKey as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_bool(), false);
}

#[test]
fn list_set_get_remove() {
    let mut code = Vec::new();
    code.push(Opcode::NewList as u8);
    code.extend_from_slice(&3u16.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&7i64.to_le_bytes());
    code.push(Opcode::SetElement as u8);
    code.push(Opcode::Dup as u8);
    code.push(0u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::RemoveKey as u8);
    code.push(Opcode::Pop as u8);
    code.push(Opcode::I64Ext as u8);
    code.push(nyar_vm::bytecode::opcode::I64Ext::Const as u8);
    code.extend_from_slice(&1i64.to_le_bytes());
    code.push(Opcode::HasKey as u8);
    code.push(Opcode::Return as u8);
    let module = minimal_module_with_chunk(code, vec![]);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_bool(), true);
}

#[test]
fn run_bootstrap_nyarc_module() {
    use nyar_vm::bytecode::decoder::Decoder;
    use std::fs;
    use std::path::Path;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let workspace = Path::new(manifest_dir)
        .parent()
        .and_then(|p| p.parent())
        .unwrap();
    let path = workspace
        .join("examples")
        .join("valkyrie-bootstrap")
        .join("target")
        .join("bootstrap.nyarc");
    if !path.exists() {
        println!("bootstrap.nyarc not found at {:?}", path);
        return;
    }
    println!("running bootstrap.nyarc at {:?}", path);
    let data = fs::read(&path).unwrap();
    let module = NyarModule::parse(&data).unwrap();
    let main_chunk = &module.chunks[0];
    let instrs = Decoder::new(&main_chunk.code).decode_all().unwrap();
    println!(
        "bootstrap main chunk: locals={} code_bytes={} instrs={}",
        main_chunk.locals,
        main_chunk.code.len(),
        instrs.len()
    );
    for (i, ins) in instrs.iter().enumerate().take(24) {
        println!("  {:04}: {:?}", i, ins);
    }

    // Scan for FFICall names and pushed file paths
    for (i, ins) in instrs.iter().enumerate() {
        match ins {
            nyar_vm::bytecode::decoder::Instruction::Push(ci) => {
                if let Some(c) = module.constants.get(*ci as usize) {
                    if let nyar_vm::bytecode::format::Constant::String(s) = c {
                        if s.ends_with(".vk") || s.contains("vcc bootstrap started") {
                            println!("  push@{:04}: {}", i, s);
                        }
                    }
                }
            }
            nyar_vm::bytecode::decoder::Instruction::FFICall(desc, argc) => {
                let name = module
                    .constants
                    .get(*desc as usize)
                    .and_then(|c| match c {
                        nyar_vm::bytecode::format::Constant::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("<non-string>");
                println!("  ffical@{:04}: {} argc={}", i, name, argc);
            }
            nyar_vm::bytecode::decoder::Instruction::InvokeMethod(mid, argc) => {
                println!("  invoke@{:04}: mid={} argc={}", i, mid, argc);
            }
            _ => {}
        }
    }

    for imp in &module.impls {
        for (mi, &chunk_idx) in imp.methods.iter().enumerate() {
            if chunk_idx as usize == 143 {
                let class = &module.classes[imp.class_idx as usize];
                let tr = &module.traits[imp.trait_idx as usize];
                let method_name = &tr.methods[mi];
                println!(
                    "chunk 143 = impl {} for {}::{}",
                    tr.name, class.name, method_name
                );
            }
        }
    }
}
