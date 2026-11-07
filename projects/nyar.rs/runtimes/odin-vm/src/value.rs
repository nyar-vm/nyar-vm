//! Runtime values — re-exported from `nyar-gc` with bytecode constant lowering.
//!
//! f32 载体合同：运行时 f32 只存在于 `Value::F32`，是独立变体，不是 `F64`/`I32` 别名。
//! 常量池当前没有 `Float32` 变体（语言层没有 f32 字面量），f32 值只能由
//! `F32ReinterpretI32` 头码按位构造或由 f64 经 `F32DemoteF64` 收窄；
//! 因此这里不存在、也不得新增「把 Float64 装进 f32」的隐式映射。

pub use nyar_gc::{CoroutineState, ObjectId, Value};

/// Converts a constant-pool entry into a runtime value.
pub fn value_from_constant(constant: &odin_bytecode::OdinConstant) -> Value {
    match constant {
        odin_bytecode::OdinConstant::Null => Value::Null,
        odin_bytecode::OdinConstant::Boolean(value) => Value::Bool(*value),
        odin_bytecode::OdinConstant::Integer32(value) => Value::I32(*value),
        odin_bytecode::OdinConstant::Integer64(value) => Value::I64(*value),
        odin_bytecode::OdinConstant::Float64(value) => Value::F64(*value),
        odin_bytecode::OdinConstant::String(value) => Value::String(value.clone()),
        odin_bytecode::OdinConstant::BigInt(_) => Value::Null,
    }
}
