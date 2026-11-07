use odin_bytecode::{OdinHeadCode, OdinInstruction};

use crate::{error::OdinRuntimeError, frame::Frame, ops::StepResult, stack::ValueStack, value::Value};

/// Integer arithmetic and comparison opcode handlers.
///
/// f32 合同（与宿主 ABI / verifier 共享同一份冻结事实）：
///
/// 1. **载体身份**：f32 值只存在于 `Value::F32`。`I32ReinterpretF32` / `F32ReinterpretI32`
///    只搬 32 位位模式，绝不经 `Value::F64`；f32 参数/局部/字段都不借用 i32/f64 槽。
/// 2. **转换规则**：`F32DemoteF64`（`v as f32`）是单步 round-to-nearest-even，
///    溢出到 `inf`、下溢到 0 或次正规；`F64PromoteF32`（`v as f64`）精确无损。
///    不存在隐式拓宽，也不做「先转 f64 再算」的降级。
/// 3. **运算与位模式**：加减乘除/比较都在 f32 上完成（不提升到 f64 再降回），
///    `F32Neg` 用 IEEE sign flip 保留 `-0.0` 与 NaN 符号，`-0.0`/次正规/NaN payload
///    在 `to_bits`/`from_bits` 往返中逐位保位。
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
        OdinHeadCode::F32Add | OdinHeadCode::F32Sub | OdinHeadCode::F32Mul | OdinHeadCode::F32Div => {
            let rhs = pop_f32(stack)?;
            let lhs = pop_f32(stack)?;
            // 全部在 f32 上完成：不提升到 f64 再降回，否则会引入 double rounding 且改变
            // 次正规/溢出边界。
            let result = match instruction.code {
                OdinHeadCode::F32Add => lhs + rhs,
                OdinHeadCode::F32Sub => lhs - rhs,
                OdinHeadCode::F32Mul => lhs * rhs,
                OdinHeadCode::F32Div => lhs / rhs,
                _ => unreachable!(),
            };
            stack.push(Value::F32(result));
        }
        OdinHeadCode::F32Neg => {
            // IEEE sign flip：保留 `-0.0` 与 NaN 符号位；不得写成 `0.0 - x`。
            let value = pop_f32(stack)?;
            stack.push(Value::F32(-value));
        }
        OdinHeadCode::F32Eq | OdinHeadCode::F32Lt => {
            let rhs = pop_f32(stack)?;
            let lhs = pop_f32(stack)?;
            let result = match instruction.code {
                OdinHeadCode::F32Eq => lhs == rhs,
                OdinHeadCode::F32Lt => lhs < rhs,
                _ => unreachable!(),
            };
            stack.push(Value::Bool(result));
        }
        OdinHeadCode::I32ReinterpretF32 => {
            let value = pop_f32(stack)?;
            // 只搬 32 位位模式：NaN payload 23 位与 `-0.0`/次正规符号位原样保留。
            stack.push(Value::I32(value.to_bits() as i32));
        }
        OdinHeadCode::F32ReinterpretI32 => {
            let bits = pop_i32(stack)? as u32;
            stack.push(Value::F32(f32::from_bits(bits)));
        }
        OdinHeadCode::F32DemoteF64 => {
            let value = match stack.pop()? {
                Value::F64(value) => value,
                other => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "f64", actual: other.type_name().to_string() });
                }
            };
            // 单步 RNE 收窄：溢出到 `inf`、下溢到 0/次正规；不经过其他中间精度。
            stack.push(Value::F32(value as f32));
        }
        OdinHeadCode::F64PromoteF32 => {
            // 单步精确拓宽：f32 的每个有限值在 f64 中都有精确表示。
            let value = pop_f32(stack)?;
            stack.push(Value::F64(value as f64));
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

/// f32 指令只接受 `Value::F32`；任何 i32/f64 值都不得冒充 f32 载体。
fn pop_f32(stack: &mut ValueStack) -> Result<f32, OdinRuntimeError> {
    match stack.pop()? {
        Value::F32(value) => Ok(value),
        other => Err(OdinRuntimeError::TypeMismatch { expected: "f32", actual: other.type_name().to_string() }),
    }
}
