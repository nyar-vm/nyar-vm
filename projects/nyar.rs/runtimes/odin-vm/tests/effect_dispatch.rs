//! PerformEffect / witness_entries dispatch round-trip tests for the odin-vm interpreter.
//!
//! These tests construct bytecode directly to verify that `PerformEffect` no longer
//! acts as a `Yield` alias when `witness_entries` are populated — instead it searches
//! the witness table for a matching `method_name`, captures the current frame as a
//! continuation, and invokes the handler function with `[continuation, effect_value]`
//! on its operand stack.

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinModuleData, OdinWitnessDispatchEntry, emit_imm1,
    emit_plain, encode_module,
};
use odin_vm::{OdinVm, Value};

/// Builds a module with:
///   function 0 "raiser": Const effect_payload, PerformEffect(method_name="raise"), Const 999, Return
///   function 1 "handler": pops [effect_value, continuation], stores continuation locally,
///     resumes continuation with effect_value, returns the resumed result
///   function 2 "main": Call 0 (triggers PerformEffect), returns result
///
/// `witness_entries` maps method_name "raise" to function 1 (handler).
/// Expected: main returns 42 (the value handler resumes the continuation with).
fn build_effect_handler_module() -> OdinModuleData {
    let mut code = Vec::new();

    // Function 0: raiser
    //   Const 0 (123)           ; push effect payload
    //   PerformEffect 1         ; operand1=1 -> constant pool[1]="raise" -> witness lookup
    //   Const 2 (999)           ; if resumed, push 999 and return it
    //   Return
    let raiser_offset = 0i32;
    emit_imm1(&mut code, OdinHeadCode::Constant, 0); // 123
    emit_imm1(&mut code, OdinHeadCode::PerformEffect, 1); // constant pool[1] = "raise"
    emit_imm1(&mut code, OdinHeadCode::Constant, 2); // 999
    emit_plain(&mut code, OdinHeadCode::Return);

    // Function 1: handler
    // Stack on entry (top -> bottom): effect_value, continuation
    //   StoreLocal 0            ; local 0 = effect_value (123)
    //   StoreLocal 1            ; local 1 = continuation
    //   LoadLocal 1             ; push continuation
    //   LoadLocal 0             ; push effect_value (as resume value)
    //   Resume                  ; resume continuation with effect_value
    //   Return                  ; return whatever the continuation returned
    let handler_offset = code.len() as i32;
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 0); // local 0 = effect_value
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 1); // local 1 = continuation
    emit_imm1(&mut code, OdinHeadCode::LoadLocal, 1); // push continuation
    emit_imm1(&mut code, OdinHeadCode::LoadLocal, 0); // push resume_value (= effect_value)
    emit_plain(&mut code, OdinHeadCode::Resume);
    emit_plain(&mut code, OdinHeadCode::Return);

    // Function 2: main
    //   Call 0                  ; call raiser -> PerformEffect -> handler invoked
    //   Return                  ; main returns whatever handler returned
    let main_offset = code.len() as i32;
    emit_imm1(&mut code, OdinHeadCode::Call, 0); // call raiser
    emit_plain(&mut code, OdinHeadCode::Return);

    OdinModuleData {
        version: Odin_VERSION,
        name: "effect_dispatch".to_string(),
        constants: vec![
            OdinConstant::Integer32(123),              // effect payload
            OdinConstant::String("raise".to_string()), // effect method_name
            OdinConstant::Integer32(999),              // raiser's return value if resumed
        ],
        functions: vec![
            OdinFunction {
                name: "raiser".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: raiser_offset,
                code_length: handler_offset - raiser_offset,
            },
            OdinFunction {
                name: "handler".to_string(),
                arity: 0,
                local_count: 2,
                code_offset: handler_offset,
                code_length: main_offset - handler_offset,
            },
            OdinFunction {
                name: "main".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: main_offset,
                code_length: code.len() as i32 - main_offset,
            },
        ],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 2 }],
        witness_entries: vec![OdinWitnessDispatchEntry {
            method_id: 0,
            type_id: 0,
            method_name: "raise".to_string(),
            function_index: 1,
            interface_id: 0,
            interface_method_index: 0,
        }],
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    }
}

#[test]
fn perform_effect_invokes_handler_from_witness_entries() {
    let module = build_effect_handler_module();
    let bytes = encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("execute main");

    // raiser raises 123 -> handler captures continuation, resumes it with 123
    // -> raiser continues after PerformEffect, pushes 999, returns 999
    // -> handler returns 999 -> main returns 999
    assert_eq!(result, Value::I32(999));
}

#[test]
fn perform_effect_without_witness_entry_falls_back_to_suspend() {
    // When witness_entries is empty, PerformEffect must degrade to the old
    // Yield-alias behavior (Suspend) so existing modules keep working.
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0); // 42
    emit_imm1(&mut code, OdinHeadCode::PerformEffect, 1); // constant pool[1] = "raise"
    emit_imm1(&mut code, OdinHeadCode::Constant, 2); // 999
    emit_plain(&mut code, OdinHeadCode::Return);

    let module = OdinModuleData {
        version: Odin_VERSION,
        name: "effect_no_witness".to_string(),
        constants: vec![OdinConstant::Integer32(42), OdinConstant::String("raise".to_string()), OdinConstant::Integer32(999)],
        functions: vec![OdinFunction { name: "raiser".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "raiser".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    let bytes = encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load module");
    let result = vm.run(&loaded, "raiser", Vec::new()).expect("execute raiser");

    // No witness entry -> PerformEffect degrades to Suspend -> top-level returns coroutine
    match result {
        Value::Coroutine(id) => {
            let state = vm.heap().get_coroutine(id).expect("coroutine in heap");
            assert!(!state.done, "suspended coroutine must not be done");
            assert_eq!(state.yielded_value, Value::I32(42), "yielded value must be effect payload");
        }
        other => panic!("expected coroutine (fallback Suspend), got {other:?}"),
    }
}

#[test]
fn handler_unwind_when_not_resuming_continuation() {
    // When handler Returns without resuming the continuation, control unwinds
    // past the PerformEffect site and the caller (main) gets the handler's return value.
    //
    // function 0 "raiser": Const 123, PerformEffect("raise"), Const 999, Return
    // function 1 "handler": StoreLocal 0 (effect_value), StoreLocal 1 (continuation),
    //   Const 3 (42), Return  -- returns 42 without resuming continuation
    // function 2 "main": Call 0, Return
    let mut code = Vec::new();
    let raiser_offset = 0i32;
    emit_imm1(&mut code, OdinHeadCode::Constant, 0); // 123
    emit_imm1(&mut code, OdinHeadCode::PerformEffect, 1); // "raise"
    emit_imm1(&mut code, OdinHeadCode::Constant, 2); // 999
    emit_plain(&mut code, OdinHeadCode::Return);

    let handler_offset = code.len() as i32;
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 0); // local 0 = effect_value
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 1); // local 1 = continuation (unused)
    emit_imm1(&mut code, OdinHeadCode::Constant, 3); // 42
    emit_plain(&mut code, OdinHeadCode::Return);

    let main_offset = code.len() as i32;
    emit_imm1(&mut code, OdinHeadCode::Call, 0);
    emit_plain(&mut code, OdinHeadCode::Return);

    let module = OdinModuleData {
        version: Odin_VERSION,
        name: "effect_unwind".to_string(),
        constants: vec![
            OdinConstant::Integer32(123),
            OdinConstant::String("raise".to_string()),
            OdinConstant::Integer32(999),
            OdinConstant::Integer32(42),
        ],
        functions: vec![
            OdinFunction {
                name: "raiser".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: raiser_offset,
                code_length: handler_offset - raiser_offset,
            },
            OdinFunction {
                name: "handler".to_string(),
                arity: 0,
                local_count: 2,
                code_offset: handler_offset,
                code_length: main_offset - handler_offset,
            },
            OdinFunction {
                name: "main".to_string(),
                arity: 0,
                local_count: 0,
                code_offset: main_offset,
                code_length: code.len() as i32 - main_offset,
            },
        ],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 2 }],
        witness_entries: vec![OdinWitnessDispatchEntry {
            method_id: 0,
            type_id: 0,
            method_name: "raise".to_string(),
            function_index: 1,
            interface_id: 0,
            interface_method_index: 0,
        }],
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    let bytes = encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("load module");
    let result = vm.run(&loaded, "main", Vec::new()).expect("execute main");

    // handler returns 42 without resuming -> main gets 42 (the raiser's continuation is discarded)
    assert_eq!(result, Value::I32(42));
}
