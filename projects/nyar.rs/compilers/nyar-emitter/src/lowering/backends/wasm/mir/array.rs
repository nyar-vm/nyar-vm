//! WASM GC 数组 operator 绑定的唯一助记符表。
//!
//! `[wasm_operator("array.*")]` 只声明目标数组操作助记符；助记符到**冻结计划**的
//! 映射归 wasm backend 所有，并在 `TargetExecutionPlan` 构造时写入计划（arity 与
//! 操作身份）。数组 operator 不是单条 opcode：`ref.cast` 与 arraytype 索引在编码期
//! 从 RepresentationPlan 注册表按元素类型解析，与 MIR `ArrayNew`/`ArrayGet`/
//! `ArraySet`/`ArrayLength` 臂共用同一发射器，计划层不冻结类型索引。
//!
//! 本模块只接受下表列出的助记符，未知助记符返回 `None`，计划据此冻结为
//! `TargetOperatorBinding::Unfrozen`，由 emit 结构化失败，禁止猜默认操作。

use crate::target_operator::{WasmGcArrayOperation, WasmGcArrayOperatorPlan};

/// 唯一助记符表。未知助记符返回 `None`。
pub(crate) fn freeze_wasm_array_binding(mnemonic: &str) -> Option<WasmGcArrayOperatorPlan> {
    Some(match mnemonic {
        // `new<T>(length) -> [T]`：按元素 arraytype 分配默认值数组。
        "array.new" => WasmGcArrayOperatorPlan { arity: 1, operation: WasmGcArrayOperation::New },
        // `length<T>([T]) -> usize`：`ref.cast array` 后 `array.len`。
        "array.length" => WasmGcArrayOperatorPlan { arity: 1, operation: WasmGcArrayOperation::Length },
        // `get<T>([T], index) -> T`：`ref.cast` 元素 arraytype 后 `array.get`。
        "array.get" => WasmGcArrayOperatorPlan { arity: 2, operation: WasmGcArrayOperation::Get },
        // `set<T>([T], index, T) -> unit`：`ref.cast` 元素 arraytype 后 `array.set`。
        "array.set" => WasmGcArrayOperatorPlan { arity: 3, operation: WasmGcArrayOperation::Set },
        _ => return None,
    })
}
