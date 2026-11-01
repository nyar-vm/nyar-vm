//! OVM 无符号比较头码（`I32LtU`/`I32LeU`/`I32GtU`/`I32GeU`）的执行语义与加载校验。
//!
//! 正向：同一对操作数 `-1` 与 `1`，无符号语义必须与有符号语义相反
//! （`-1` 的位型 `0xFFFFFFFF` 不小于 `1`）；非负操作数上与有符号语义一致。
//! 反向：未分配槽位（如 `0x4A`）必须在加载期被校验拒绝，不得进入解释循环。
use odin_bytecode::{Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinModuleData, encode_module};
use odin_vm::{executor::Executor, module::LoadedModule, *};

fn comparison_bytes(head: u8, lhs: i32, rhs: i32) -> Vec<u8> {
    let code = vec![OdinHeadCode::Constant as u8, 0, 0, 0, 0, OdinHeadCode::Constant as u8, 1, 0, 0, 0, head, OdinHeadCode::Return as u8];
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "unsigned_cmp".to_string(),
        constants: vec![OdinConstant::Integer32(lhs), OdinConstant::Integer32(rhs)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: code.len() as i32 }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    encode_module(&data)
}

fn run(head: u8, lhs: i32, rhs: i32) -> Value {
    let bytes = comparison_bytes(head, lhs, rhs);
    let module = LoadedModule::from_bytes(&bytes).expect("无符号比较头码必须通过加载校验");
    let mut executor = Executor::new();
    executor.run(&module, 0, Vec::new()).expect("执行")
}

#[test]
fn unsigned_comparisons_use_unsigned_bit_patterns() {
    // `-1` 按无符号读出是 `0xFFFFFFFF`：结果必须与有符号比较相反。
    for (op, expected) in
        [(OdinHeadCode::I32LtU, false), (OdinHeadCode::I32LeU, false), (OdinHeadCode::I32GtU, true), (OdinHeadCode::I32GeU, true)]
    {
        assert_eq!(run(op as u8, -1, 1), Value::Bool(expected), "{op:?} 对 (-1, 1) 的无符号语义");
    }
    // 非负操作数上无符号与有符号语义一致。
    for (op, expected) in
        [(OdinHeadCode::I32LtU, true), (OdinHeadCode::I32LeU, true), (OdinHeadCode::I32GtU, false), (OdinHeadCode::I32GeU, false)]
    {
        assert_eq!(run(op as u8, 3, 5), Value::Bool(expected), "{op:?} 对 (3, 5)");
    }
}

#[test]
fn unallocated_comparison_slot_is_rejected_at_load() {
    let bytes = comparison_bytes(0x4A, -1, 1);
    assert!(LoadedModule::from_bytes(&bytes).is_err(), "未分配头码 0x4A 必须在加载期结构化失败");
}
