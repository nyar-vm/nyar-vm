use odin_bytecode::{OdinHeadCode, OdinInstruction};

use crate::{error::OdinRuntimeError, frame::Frame, ops::StepResult, stack::ValueStack, value::Value};

/// Integer arithmetic and comparison opcode handlers.
pub fn execute_arithmetic(instruction: OdinInstruction, frame: &mut Frame, stack: &mut ValueStack) -> Result<StepResult, OdinRuntimeError> {
    match instruction.code {
        OdinHeadCode::I32Add | OdinHeadCode::I32Sub | OdinHeadCode::I32Mul | OdinHeadCode::I32DivS | OdinHeadCode::I32RemS => {
            let rhs = pop_i32(stack)?;
            let lhs = pop_i32(stack)?;
            let result = match instruction.code {
                OdinHeadCode::I32Add => lhs.wrapping_add(rhs),
                OdinHeadCode::I32Sub => lhs.wrapping_sub(rhs),
                OdinHeadCode::I32Mul => lhs.wrapping_mul(rhs),
                OdinHeadCode::I32DivS => {
                    if rhs == 0 {
                        return Err(OdinRuntimeError::DivisionByZero { operation: "i32.div_s" });
                    }
                    // `i32::MIN / -1` 在数学上溢出 i32 表示范围：不得 panic 或回绕，转结构化错误。
                    lhs.checked_div(rhs).ok_or(OdinRuntimeError::ArithmeticOverflow { operation: "i32.div_s" })?
                }
                OdinHeadCode::I32RemS => {
                    if rhs == 0 {
                        return Err(OdinRuntimeError::DivisionByZero { operation: "i32.rem_s" });
                    }
                    // `i32::MIN % -1` 与除法同源溢出（Rust `wrapping_rem` 会静默给 0）；统一结构化失败。
                    lhs.checked_rem(rhs).ok_or(OdinRuntimeError::ArithmeticOverflow { operation: "i32.rem_s" })?
                }
                _ => unreachable!(),
            };
            stack.push(Value::I32(result));
        }
        OdinHeadCode::I32And
        | OdinHeadCode::I32Or
        | OdinHeadCode::I32Xor
        | OdinHeadCode::I32Shl
        | OdinHeadCode::I32ShrS
        | OdinHeadCode::I32ShrU => {
            let rhs = pop_i32(stack)?;
            let lhs = pop_i32(stack)?;
            let result = match instruction.code {
                OdinHeadCode::I32And => lhs & rhs,
                OdinHeadCode::I32Or => lhs | rhs,
                OdinHeadCode::I32Xor => lhs ^ rhs,
                OdinHeadCode::I32Shl => lhs.wrapping_shl(rhs as u32),
                OdinHeadCode::I32ShrS => lhs.wrapping_shr(rhs as u32),
                OdinHeadCode::I32ShrU => ((lhs as u32).wrapping_shr(rhs as u32)) as i32,
                _ => unreachable!(),
            };
            stack.push(Value::I32(result));
        }
        OdinHeadCode::I32Eq
        | OdinHeadCode::I32Ne
        | OdinHeadCode::I32LtS
        | OdinHeadCode::I32LtU
        | OdinHeadCode::I32LeS
        | OdinHeadCode::I32LeU
        | OdinHeadCode::I32GtS
        | OdinHeadCode::I32GtU
        | OdinHeadCode::I32GeS
        | OdinHeadCode::I32GeU => {
            let rhs = pop_i32(stack)?;
            let lhs = pop_i32(stack)?;
            let result = match instruction.code {
                OdinHeadCode::I32Eq => lhs == rhs,
                OdinHeadCode::I32Ne => lhs != rhs,
                OdinHeadCode::I32LtS => lhs < rhs,
                OdinHeadCode::I32LtU => (lhs as u32) < (rhs as u32),
                OdinHeadCode::I32LeS => lhs <= rhs,
                OdinHeadCode::I32LeU => (lhs as u32) <= (rhs as u32),
                OdinHeadCode::I32GtS => lhs > rhs,
                OdinHeadCode::I32GtU => (lhs as u32) > (rhs as u32),
                OdinHeadCode::I32GeS => lhs >= rhs,
                OdinHeadCode::I32GeU => (lhs as u32) >= (rhs as u32),
                _ => unreachable!(),
            };
            stack.push(Value::Bool(result));
        }
        OdinHeadCode::I64ReinterpretF64 => {
            let value = match stack.pop()? {
                Value::F64(value) => value,
                other => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "f64", actual: other.type_name().to_string() });
                }
            };
            // 只搬位：`to_bits` 保留符号位、次正规数与 NaN payload，不做数值转换。
            stack.push(Value::I64(value.to_bits() as i64));
        }
        OdinHeadCode::F64ReinterpretI64 => {
            let bits = match stack.pop()? {
                Value::I64(bits) => bits as u64,
                other => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "i64", actual: other.type_name().to_string() });
                }
            };
            // 只搬位：`from_bits` 逐位重建，含 `-0.0`、次正规数与 NaN payload。
            stack.push(Value::F64(f64::from_bits(bits)));
        }
        _ => return Err(OdinRuntimeError::UnknownOpcode(instruction.code as u8)),
    }

    frame.ip += instruction.size as usize;
    Ok(StepResult::Continue)
}

fn pop_i32(stack: &mut ValueStack) -> Result<i32, OdinRuntimeError> {
    match stack.pop()? {
        Value::I32(value) => Ok(value),
        other => Err(OdinRuntimeError::TypeMismatch { expected: "i32", actual: other.type_name().to_string() }),
    }
}
