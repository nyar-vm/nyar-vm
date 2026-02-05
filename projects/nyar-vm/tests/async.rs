use nyar_types::{NyarError, QualifiedName};
use nyar_vm::bytecode::format::{Chunk, Constant, NyarModule};
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::vm::core::NyarVM;

#[test]
fn run_await_on_closure() {
    let mut callee = Vec::new();
    callee.push(Opcode::Push as u8);
    callee.extend_from_slice(&0u16.to_le_bytes());
    callee.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(0u8);
    main.push(Opcode::Await as u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![Constant::Int(42)],
        chunks: vec![
            Chunk {
                max_stack: 8,
                code: callee,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 42);
}

#[test]
fn run_block_on_closure() {
    let mut callee = Vec::new();
    callee.push(Opcode::Push as u8);
    callee.extend_from_slice(&0u16.to_le_bytes());
    callee.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(0u8);
    main.push(Opcode::BlockOn as u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![Constant::Int(7)],
        chunks: vec![
            Chunk {
                max_stack: 8,
                code: callee,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 7);
}

#[test]
fn run_try_raise_catch_effect() {
    let mut catch = Vec::new();
    catch.push(Opcode::MatchEffect as u8);
    catch.extend_from_slice(&0u16.to_le_bytes());
    catch.push(Opcode::JumpIfFalse as u8);
    catch.extend_from_slice(&(5i16).to_le_bytes());
    catch.push(Opcode::LoadLocal as u8);
    catch.push(1u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&1u16.to_le_bytes());
    catch.push(Opcode::GetElement as u8);
    catch.push(Opcode::Return as u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&2u16.to_le_bytes());
    catch.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&3u16.to_le_bytes());
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["boom".to_string()])),
            Constant::Int(0),
            Constant::Int(0),
            Constant::Int(1),
        ],
        effects: vec![QualifiedName::new(vec!["boom".to_string()])],
        chunks: vec![
            Chunk {
                locals: 2,
                max_stack: 8,
                code: catch,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 1);
}

#[test]
fn run_algebraic_effect_xxeffect() {
    // catch chunk: if MatchEffect("XXeffect") then return locals[1][0], else return 0
    let mut catch = Vec::new();
    catch.push(Opcode::MatchEffect as u8);
    catch.extend_from_slice(&0u16.to_le_bytes());
    catch.push(Opcode::JumpIfFalse as u8);
    catch.extend_from_slice(&(5i16).to_le_bytes());
    catch.push(Opcode::LoadLocal as u8);
    catch.push(1u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&1u16.to_le_bytes()); // index 0
    catch.push(Opcode::GetElement as u8);
    catch.push(Opcode::Return as u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&2u16.to_le_bytes()); // fallback 0
    catch.push(Opcode::Return as u8);

    // main chunk: WithHandler(catch), raise XXeffect(42), return
    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&3u16.to_le_bytes()); // 42
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes()); // effects[0] = "XXeffect"
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["XXeffect".to_string()])),
            Constant::Int(0), // index 0 for args list
            Constant::Int(0), // fallback 0
            Constant::Int(42),
        ],
        effects: vec![QualifiedName::new(vec!["XXeffect".to_string()])],
        chunks: vec![
            Chunk {
                locals: 2,
                max_stack: 8,
                code: catch,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 42);
}

#[test]
fn run_perform_await_on_closure() {
    let mut callee = Vec::new();
    callee.push(Opcode::Push as u8);
    callee.extend_from_slice(&1u16.to_le_bytes());
    callee.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::MakeClosure as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(0u8);
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["await".to_string()])),
            Constant::Int(99),
        ],
        effects: vec![QualifiedName::new(vec!["await".to_string()])],
        chunks: vec![
            Chunk {
                max_stack: 8,
                code: callee,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 99);
}

#[test]
fn run_effect_handler_resume_with_continuation() {
    let mut catch_x = Vec::new();
    catch_x.push(Opcode::MatchEffect as u8);
    catch_x.extend_from_slice(&0u16.to_le_bytes());
    catch_x.push(Opcode::JumpIfFalse as u8);
    catch_x.extend_from_slice(&(8i16).to_le_bytes());
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(2u8);
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(1u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&2u16.to_le_bytes());
    catch_x.push(Opcode::GetElement as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&3u16.to_le_bytes());
    catch_x.push(Opcode::InvokeMethod as u8);
    catch_x.extend_from_slice(&1u16.to_le_bytes());
    catch_x.push(1u8);
    catch_x.push(Opcode::ResumeWith as u8);
    catch_x.push(Opcode::Return as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&4u16.to_le_bytes());
    catch_x.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&5u16.to_le_bytes());
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["XXeffect".to_string()])),
            Constant::String("add".to_string()),
            Constant::Int(0),
            Constant::Int(1),
            Constant::Int(0),
            Constant::Int(10),
        ],
        effects: vec![QualifiedName::new(vec!["XXeffect".to_string()])],
        chunks: vec![
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch_x,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 11);
}

#[test]
fn run_effect_multi_layer_two_effects() {
    let mut catch_x = Vec::new();
    catch_x.push(Opcode::MatchEffect as u8);
    catch_x.extend_from_slice(&0u16.to_le_bytes());
    catch_x.push(Opcode::JumpIfFalse as u8);
    catch_x.extend_from_slice(&(8i16).to_le_bytes());
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(2u8);
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(1u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&3u16.to_le_bytes());
    catch_x.push(Opcode::GetElement as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&4u16.to_le_bytes());
    catch_x.push(Opcode::InvokeMethod as u8);
    catch_x.extend_from_slice(&2u16.to_le_bytes());
    catch_x.push(1u8);
    catch_x.push(Opcode::ResumeWith as u8);
    catch_x.push(Opcode::Return as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&8u16.to_le_bytes());
    catch_x.push(Opcode::Return as u8);

    let mut catch_y = Vec::new();
    catch_y.push(Opcode::MatchEffect as u8);
    catch_y.extend_from_slice(&1u16.to_le_bytes());
    catch_y.push(Opcode::JumpIfFalse as u8);
    catch_y.extend_from_slice(&(8i16).to_le_bytes());
    catch_y.push(Opcode::LoadLocal as u8);
    catch_y.push(2u8);
    catch_y.push(Opcode::LoadLocal as u8);
    catch_y.push(1u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&3u16.to_le_bytes());
    catch_y.push(Opcode::GetElement as u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&5u16.to_le_bytes());
    catch_y.push(Opcode::InvokeMethod as u8);
    catch_y.extend_from_slice(&2u16.to_le_bytes());
    catch_y.push(1u8);
    catch_y.push(Opcode::ResumeWith as u8);
    catch_y.push(Opcode::Return as u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&8u16.to_le_bytes());
    catch_y.push(Opcode::Return as u8);

    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&6u16.to_le_bytes());
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&7u16.to_le_bytes());
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::InvokeMethod as u8);
    main.extend_from_slice(&2u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["XXeffect".to_string()])),
            Constant::QualifiedName(QualifiedName::new(vec!["YYeffect".to_string()])),
            Constant::String("add".to_string()),
            Constant::Int(0),
            Constant::Int(1),
            Constant::Int(2),
            Constant::Int(10),
            Constant::Int(20),
            Constant::Int(0),
        ],
        effects: vec![
            QualifiedName::new(vec!["XXeffect".to_string()]),
            QualifiedName::new(vec!["YYeffect".to_string()]),
        ],
        chunks: vec![
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch_x,
                ..Default::default()
            },
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch_y,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 2).unwrap();
    assert_eq!(v.as_int(), 33);
}

#[test]
fn run_effect_propagates_to_next_handler() {
    // first handler matches YYeffect, will not be chosen
    let mut catch_y = Vec::new();
    catch_y.push(Opcode::MatchEffect as u8);
    catch_y.extend_from_slice(&1u16.to_le_bytes()); // YYeffect
    catch_y.push(Opcode::JumpIfFalse as u8);
    catch_y.extend_from_slice(&(8i16).to_le_bytes());
    catch_y.push(Opcode::LoadLocal as u8);
    catch_y.push(2u8);
    catch_y.push(Opcode::LoadLocal as u8);
    catch_y.push(1u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&3u16.to_le_bytes());
    catch_y.push(Opcode::GetElement as u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&5u16.to_le_bytes()); // +2
    catch_y.push(Opcode::InvokeMethod as u8);
    catch_y.extend_from_slice(&2u16.to_le_bytes()); // add
    catch_y.push(1u8);
    catch_y.push(Opcode::ResumeWith as u8);
    catch_y.push(Opcode::Return as u8);
    catch_y.push(Opcode::Push as u8);
    catch_y.extend_from_slice(&8u16.to_le_bytes()); // fallback 0
    catch_y.push(Opcode::Return as u8);

    // second handler matches XXeffect, should be chosen
    let mut catch_x = Vec::new();
    catch_x.push(Opcode::MatchEffect as u8);
    catch_x.extend_from_slice(&0u16.to_le_bytes()); // XXeffect
    catch_x.push(Opcode::JumpIfFalse as u8);
    catch_x.extend_from_slice(&(8i16).to_le_bytes());
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(2u8);
    catch_x.push(Opcode::LoadLocal as u8);
    catch_x.push(1u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&3u16.to_le_bytes());
    catch_x.push(Opcode::GetElement as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&4u16.to_le_bytes()); // +3
    catch_x.push(Opcode::InvokeMethod as u8);
    catch_x.extend_from_slice(&2u16.to_le_bytes()); // add
    catch_x.push(1u8);
    catch_x.push(Opcode::ResumeWith as u8);
    catch_x.push(Opcode::Return as u8);
    catch_x.push(Opcode::Push as u8);
    catch_x.extend_from_slice(&8u16.to_le_bytes()); // fallback 0
    catch_x.push(Opcode::Return as u8);

    // main: WithHandler(catch_y), WithHandler(catch_x), raise XXeffect(7) -> 10
    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&6u16.to_le_bytes()); // 7
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes()); // XXeffect
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["XXeffect".to_string()])), // 0
            Constant::QualifiedName(QualifiedName::new(vec!["YYeffect".to_string()])), // 1
            Constant::String("add".to_string()),      // 2
            Constant::Int(0),                         // 3 index 0
            Constant::Int(3),                         // 4 +3
            Constant::Int(2),                         // 5 +2
            Constant::Int(7),                         // 6 arg
            Constant::Int(0),                         // 7 unused
            Constant::Int(0),                         // 8 fallback 0
        ],
        effects: vec![
            QualifiedName::new(vec!["XXeffect".to_string()]),
            QualifiedName::new(vec!["YYeffect".to_string()]),
        ],
        chunks: vec![
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch_x,
                ..Default::default()
            },
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch_y,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 2).unwrap();
    assert_eq!(v.as_int(), 10);
}

#[test]
fn run_effect_unhandled_error_top_level() {
    // main: raise XXeffect(1) without any handlers -> error
    let mut main = Vec::new();
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes()); // 1
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes()); // XXeffect
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![Constant::Int(1)],
        effects: vec![QualifiedName::new(vec!["XXeffect".to_string()])],
        chunks: vec![Chunk {
            max_stack: 8,
            code: main,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let r = vm.execute(module_idx, 0);
    assert!(r.is_err());
}

#[test]
fn run_throw_effect_catch_returns() {
    // catch: if MatchEffect("throw") then return 123; else return 0
    let mut catch = Vec::new();
    catch.push(Opcode::MatchEffect as u8);
    catch.extend_from_slice(&0u16.to_le_bytes()); // "throw"
    catch.push(Opcode::JumpIfFalse as u8);
    catch.extend_from_slice(&(3i16).to_le_bytes());
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&2u16.to_le_bytes()); // 123
    catch.push(Opcode::Return as u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&3u16.to_le_bytes()); // 0
    catch.push(Opcode::Return as u8);

    // main: WithHandler(catch), Perform("throw", 1) -> handler returns 123
    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes()); // 1
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes()); // "throw"
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["throw".to_string()])), // 0
            Constant::Int(1),                      // 1 arg
            Constant::Int(123),                    // 2 expected
            Constant::Int(0),                      // 3 fallback
        ],
        effects: vec![QualifiedName::new(vec!["throw".to_string()])],
        chunks: vec![
            Chunk {
                locals: 1,
                max_stack: 8,
                code: catch,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 123);
}

#[test]
fn run_throw_effect_uncaught_is_unhandled_error() {
    // perform throw without handler -> VmError::UnhandledError
    let mut main = Vec::new();
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes()); // 1
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes()); // "throw"
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["throw".to_string()])),
            Constant::Int(1),
        ],
        effects: vec![QualifiedName::new(vec!["throw".to_string()])],
        chunks: vec![Chunk {
            max_stack: 8,
            code: main,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    let r = vm.execute(module_idx, 0);
    match r {
        Err(_) => {}
        _ => panic!("expected error for uncaught throw"),
    }
}

#[test]
fn run_throw_effect_uncaught_prints_traceback() {
    let mut main = Vec::new();
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["throw".to_string()])),
            Constant::Int(1),
        ],
        effects: vec![QualifiedName::new(vec!["throw".to_string()])],
        chunks: vec![Chunk {
            max_stack: 8,
            code: main,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    use std::cell::RefCell;
    use std::rc::Rc;
    let output = Rc::new(RefCell::new(Vec::<String>::new()));
    let out_clone = output.clone();
    vm.stdout = Some(Box::new(move |msg: &str| {
        out_clone.borrow_mut().push(msg.to_string());
    }));
    let r = vm.execute(module_idx, 0);
    assert!(r.is_err());
    let err = NyarError::new(
        0x1001,
        nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::RuntimeError("UnhandledError".to_string())),
        nyar_types::SourceLocation::default(),
    );
    vm.print_traceback(&err);
    let lines = output.borrow();
    assert!(!lines.is_empty());
    assert_eq!(lines[0], "Traceback (most recent call last):");
    assert!(lines.last().unwrap().contains("UnhandledError"));
}
#[test]
fn run_logger_event_default_prints() {
    // main: StringConst("hello"), Perform("LoggerEvent", 1), Push 0, Return
    let mut main = Vec::new();
    main.push(Opcode::StringExt as u8);
    main.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    main.push(5u8);
    main.extend_from_slice(b"hello");
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["LoggerEvent".to_string()])),
            Constant::Int(0),
            Constant::Int(0),
        ],
        effects: vec![QualifiedName::new(vec!["LoggerEvent".to_string()])],
        chunks: vec![Chunk {
            max_stack: 8,
            code: main,
            ..Default::default()
        }],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    use std::cell::RefCell;
    use std::rc::Rc;
    let output = Rc::new(RefCell::new(Vec::<String>::new()));
    let out_clone = output.clone();
    vm.stdout = Some(Box::new(move |msg: &str| {
        out_clone.borrow_mut().push(msg.to_string());
    }));
    let v = vm.execute(module_idx, 0).unwrap();
    assert_eq!(v.as_int(), 0);
    let lines = output.borrow();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "hello");
}

#[test]
fn run_logger_event_handler_prints_and_resumes() {
    // catch: if MatchEffect("LoggerEvent"), print first arg, pop null, push 0, resume
    let mut catch = Vec::new();
    catch.push(Opcode::MatchEffect as u8);
    catch.extend_from_slice(&0u16.to_le_bytes());
    catch.push(Opcode::JumpIfFalse as u8);
    catch.extend_from_slice(&(9i16).to_le_bytes());
    catch.push(Opcode::LoadLocal as u8);
    catch.push(2u8);
    catch.push(Opcode::LoadLocal as u8);
    catch.push(1u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&1u16.to_le_bytes());
    catch.push(Opcode::GetElement as u8);
    catch.push(Opcode::FFICall as u8);
    catch.extend_from_slice(&2u16.to_le_bytes());
    catch.push(1u8);
    catch.push(Opcode::Pop as u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&1u16.to_le_bytes());
    catch.push(Opcode::ResumeWith as u8);
    catch.push(Opcode::Return as u8);
    catch.push(Opcode::Push as u8);
    catch.extend_from_slice(&1u16.to_le_bytes());
    catch.push(Opcode::Return as u8);

    // main: WithHandler(catch), StringConst("x"), Perform("LoggerEvent", 1), Push 1, Return
    let mut main = Vec::new();
    main.push(Opcode::WithHandler as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(Opcode::StringExt as u8);
    main.push(nyar_vm::bytecode::opcode::StringExt::Const as u8);
    main.push(1u8);
    main.extend_from_slice(b"x");
    main.push(Opcode::Perform as u8);
    main.extend_from_slice(&0u16.to_le_bytes());
    main.push(1u8);
    main.push(Opcode::Pop as u8);
    main.push(Opcode::Push as u8);
    main.extend_from_slice(&1u16.to_le_bytes());
    main.push(Opcode::Return as u8);

    let module = NyarModule {
        constants: vec![
            Constant::QualifiedName(QualifiedName::new(vec!["LoggerEvent".to_string()])),
            Constant::Int(0),
            Constant::String("print".to_string()),
        ],
        effects: vec![QualifiedName::new(vec!["LoggerEvent".to_string()])],
        chunks: vec![
            Chunk {
                locals: 3,
                max_stack: 8,
                code: catch,
                ..Default::default()
            },
            Chunk {
                max_stack: 8,
                code: main,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    use std::cell::RefCell;
    use std::rc::Rc;
    let output = Rc::new(RefCell::new(Vec::<String>::new()));
    let out_clone = output.clone();
    vm.stdout = Some(Box::new(move |msg: &str| {
        out_clone.borrow_mut().push(msg.to_string());
    }));
    let v = vm.execute(module_idx, 1).unwrap();
    assert_eq!(v.as_int(), 0);
    let lines = output.borrow();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0], "x");
}
