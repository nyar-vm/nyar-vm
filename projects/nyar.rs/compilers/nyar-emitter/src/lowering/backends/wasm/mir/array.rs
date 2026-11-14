//! WASM GC 数组指令绑定的唯一助记符表。
//!
//! `[wasm_operator("array.get")]` 只声明目标操作身份；助记符到**冻结计划**的映射归
//! wasm backend 所有，并在 `TargetExecutionPlan` 构造时一次性写入计划（arity 与操作
//! 身份）。数组 operator 是多步编码（`ref.cast` + `array.get`/`array.set`/
//! `array.new_default`/`array.len`），arraytype 索引随元素类型在编码期从
//! RepresentationPlan 注册表解析——与 MIR `ArrayNew`/`ArrayGet`/`ArraySet`/
//! `ArrayLength` 臂共用同一发射入口，本表不冻结任何类型索引。
//!
//! 本模块只接受下表列出的助记符，未知助记符返回 `None`，计划据此冻结为
//! `对应 lane 的 `Unfrozen` 绑定`，由 emit 结构化失败，禁止猜默认指令。

use crate::target_operator::{WasmGcArrayOperation, WasmGcArrayOperatorPlan};

const fn array_plan(arity: usize, operation: WasmGcArrayOperation) -> WasmGcArrayOperatorPlan {
    WasmGcArrayOperatorPlan { arity, operation }
}

/// 唯一数组助记符表。未知助记符返回 `None`。
pub(crate) fn freeze_wasm_array_binding(mnemonic: &str) -> Option<WasmGcArrayOperatorPlan> {
    Some(match mnemonic {
        "array.new" => array_plan(1, WasmGcArrayOperation::New),
        "array.length" => array_plan(1, WasmGcArrayOperation::Length),
        "array.get" => array_plan(2, WasmGcArrayOperation::Get),
        "array.set" => array_plan(3, WasmGcArrayOperation::Set),
        _ => return None,
    })
}
