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
                    lhs.wrapping_div(rhs)
                }
                OdinHeadCode::I32RemS => {
                    if rhs == 0 {
                        return Err(OdinRuntimeError::DivisionByZero { operation: "i32.rem_s" });
                    }
                    lhs.wrapping_rem(rhs)
                }
                _ => unreachable!(),
            };
            stack.push(Value::I32(result));
        }
        OdinHeadCode::I32Eq
        | OdinHeadCode::I32Ne
        | OdinHeadCode::I32LtS
        | OdinHeadCode::I32LeS
        | OdinHeadCode::I32GtS
        | OdinHeadCode::I32GeS => {
            let rhs = pop_i32(stack)?;
            let lhs = pop_i32(stack)?;
            let result = match instruction.code {
                OdinHeadCode::I32Eq => lhs == rhs,
                OdinHeadCode::I32Ne => lhs != rhs,
                OdinHeadCode::I32LtS => lhs < rhs,
                OdinHeadCode::I32LeS => lhs <= rhs,
                OdinHeadCode::I32GtS => lhs > rhs,
                OdinHeadCode::I32GeS => lhs >= rhs,
                _ => unreachable!(),
            };
            stack.push(Value::Bool(result));
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
