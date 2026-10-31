//! 结构指令：`ObjectNew` / `FieldGet` / `FieldSet`（按 layout_id + field_slot）。

use odin_bytecode::{OdinHeadCode, OdinInstruction};

use nyar_gc::ObjectPayload;

use crate::{
    error::OdinRuntimeError,
    frame::Frame,
    ops::{ExecutionContext, StepResult},
    value::Value,
};

/// 执行结构指令族。
pub fn execute_object(instruction: OdinInstruction, frame: &mut Frame, ctx: &mut ExecutionContext<'_>) -> Result<StepResult, OdinRuntimeError> {
    match instruction.code {
        OdinHeadCode::ObjectNew => {
            let layout_id = instruction.operand1;
            if layout_id < 0 || (layout_id as usize) >= ctx.module.layouts.len() {
                return Err(OdinRuntimeError::LayoutIndexOutOfRange(layout_id));
            }
            let field_count = ctx.module.layouts[layout_id as usize].field_count;
            if field_count < 0 {
                return Err(OdinRuntimeError::ModuleLoad(format!("layout[{layout_id}] has negative field_count")));
            }
            let object_id = ctx.heap.alloc_layout_object(layout_id as u32, field_count as usize);
            ctx.stack.push(Value::Object(object_id));
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::FieldGet => {
            let field_slot = instruction.operand1;
            let object = ctx.stack.pop()?;
            let object_id = match object {
                Value::Object(id) => id,
                other => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "object", actual: other.type_name().to_string() });
                }
            };
            let value = match ctx.heap.get(object_id) {
                Some(ObjectPayload::LayoutObject { slots, .. }) => {
                    if field_slot < 0 || (field_slot as usize) >= slots.len() {
                        return Err(OdinRuntimeError::FieldSlotOutOfRange(field_slot));
                    }
                    slots[field_slot as usize].clone()
                }
                Some(_) => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "layout object", actual: "non-layout object".to_string() });
                }
                None => {
                    return Err(OdinRuntimeError::ModuleLoad(format!("object heap id {object_id} not found")));
                }
            };
            ctx.stack.push(value);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::FieldSet => {
            let field_slot = instruction.operand1;
            if field_slot < 0 {
                return Err(OdinRuntimeError::FieldSlotOutOfRange(field_slot));
            }
            let value = ctx.stack.pop()?;
            let object = ctx.stack.pop()?;
            let object_id = match object {
                Value::Object(id) => id,
                other => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "object", actual: other.type_name().to_string() });
                }
            };
            if ctx.heap.get(object_id).is_none() {
                return Err(OdinRuntimeError::ModuleLoad(format!("object heap id {object_id} not found")));
            }
            match ctx.heap.set_field(object_id, field_slot as usize, value) {
                Ok(()) => {}
                Err("field slot out of range") => {
                    return Err(OdinRuntimeError::FieldSlotOutOfRange(field_slot));
                }
                Err(_) => {
                    return Err(OdinRuntimeError::TypeMismatch { expected: "layout object", actual: "non-layout object".to_string() });
                }
            }
            ctx.stack.push(Value::Object(object_id));
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        _ => Err(OdinRuntimeError::UnknownOpcode(instruction.code as u8)),
    }
}
