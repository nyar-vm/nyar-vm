//! Runtime values — re-exported from `nyar-gc` with bytecode constant lowering.

pub use nyar_gc::{CoroutineState, ObjectId, Value};

/// Converts a constant-pool entry into a runtime value.
pub fn value_from_constant(constant: &odin_bytecode::OdinConstant) -> Value {
    match constant {
        odin_bytecode::OdinConstant::Null => Value::Null,
        odin_bytecode::OdinConstant::Boolean(value) => Value::Bool(*value),
        odin_bytecode::OdinConstant::Integer32(value) => Value::I32(*value),
        odin_bytecode::OdinConstant::Float64(value) => Value::F64(*value),
        odin_bytecode::OdinConstant::String(value) => Value::String(value.clone()),
        odin_bytecode::OdinConstant::BigInt(_) => Value::Null,
    }
}
