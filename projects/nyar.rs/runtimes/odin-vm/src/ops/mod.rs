//! 内码操作分派：算术、控制流、对象与宿主调用。

use odin_bytecode::OdinHeadCode;

use crate::{
    error::OdinRuntimeError,
    executable::ExecOp,
    frame::Frame,
    module::LoadedModule,
    stack::ValueStack,
    value::{CoroutineState, ObjectId, Value, value_from_constant},
};
use nyar_gc::ObjectHeap;

mod arithmetic;
mod control;
mod object;

pub use arithmetic::execute_arithmetic;
pub use control::{execute_call, execute_control};
pub use object::execute_object;

/// Result of executing one instruction.
#[derive(Debug, Clone, PartialEq)]
pub enum StepResult {
    /// Continue with the current frame.
    Continue,
    /// Return from the current frame.
    Return,
    /// Call a nested frame.
    Call {
        /// Target function index.
        function_index: usize,
    },
    /// Suspend the current frame: pop a yielded value and capture the frame as a coroutine.
    Suspend {
        /// The value yielded to the caller.
        yielded_value: Value,
    },
    /// Resume a coroutine: restore a suspended frame and inject the resume value.
    ResumeCoroutine {
        /// Heap id of the coroutine being resumed.
        coroutine_id: ObjectId,
        /// The captured frame snapshot to restore.
        state: CoroutineState,
        /// The value injected into the resumed coroutine.
        resume_value: Value,
    },
    /// Invoke an effect handler found via `witness_entries`.
    InvokeHandler {
        /// Handler function index from `witness_entries`.
        handler_function_index: usize,
        /// The effect payload value popped from the stack.
        effect_value: Value,
    },
}

/// Execution context passed to opcode handlers.
pub struct ExecutionContext<'a> {
    /// Loaded module metadata.
    pub module: &'a LoadedModule,
    /// Module-level global slots.
    pub globals: &'a mut [Value],
    /// Operand stack.
    pub stack: &'a mut ValueStack,
    /// Object heap.
    pub heap: &'a mut ObjectHeap,
}

/// 执行一条预解码内码；`frame.ip` 为函数内指令下标。
pub fn dispatch_exec(op: ExecOp, frame: &mut Frame, ctx: &mut ExecutionContext<'_>) -> Result<StepResult, OdinRuntimeError> {
    match op.code {
        OdinHeadCode::Jump => {
            frame.ip = op.target as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::JumpIfTrue | OdinHeadCode::JumpIfFalse => {
            let condition = ctx.stack.pop()?.to_bool();
            let should_jump = match op.code {
                OdinHeadCode::JumpIfTrue => condition,
                OdinHeadCode::JumpIfFalse => !condition,
                _ => false,
            };
            if should_jump {
                frame.ip = op.target as usize;
            }
            else {
                frame.ip += 1;
            }
            Ok(StepResult::Continue)
        }
        OdinHeadCode::Nop => {
            frame.ip += 1;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::Constant => {
            let index = op.operand1;
            let constant = ctx.module.constant_at(index).ok_or(OdinRuntimeError::ConstantIndexOutOfRange(index))?;
            ctx.stack.push(value_from_constant(constant));
            frame.ip += 1;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::Pop => {
            ctx.stack.pop()?;
            frame.ip += 1;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::Dup => {
            ctx.stack.dup()?;
            frame.ip += 1;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::Return => Ok(StepResult::Return),
        OdinHeadCode::Yield => {
            let yielded_value = ctx.stack.pop()?;
            frame.ip += 1;
            Ok(StepResult::Suspend { yielded_value })
        }
        OdinHeadCode::Resume => {
            let resume_value = ctx.stack.pop()?;
            let coroutine = ctx.stack.pop()?;
            frame.ip += 1;
            match coroutine {
                Value::Coroutine(id) => {
                    let state = ctx.heap.get_coroutine(id).ok_or(OdinRuntimeError::ModuleLoad(format!("coroutine heap id {id} not found")))?;
                    if state.done {
                        return Err(OdinRuntimeError::TypeMismatch { expected: "active coroutine", actual: "completed coroutine".to_string() });
                    }
                    Ok(StepResult::ResumeCoroutine { coroutine_id: id, state: state.clone(), resume_value })
                }
                other => Err(OdinRuntimeError::TypeMismatch { expected: "coroutine", actual: other.type_name().to_string() }),
            }
        }
        OdinHeadCode::PerformEffect => {
            let effect_value = ctx.stack.pop()?;
            frame.ip += 1;

            let effect_name_index = op.operand1;
            let effect_name = match ctx.module.constant_at(effect_name_index) {
                Some(odin_bytecode::OdinConstant::String(name)) => name.as_str(),
                _ => "raise",
            };

            let handler_entry = ctx.module.witness_entries.iter().find(|entry| entry.method_name == effect_name);

            match handler_entry {
                Some(entry) if entry.function_index >= 0 => {
                    Ok(StepResult::InvokeHandler { handler_function_index: entry.function_index as usize, effect_value })
                }
                _ => Ok(StepResult::Suspend { yielded_value: effect_value }),
            }
        }
        OdinHeadCode::Call | OdinHeadCode::CallStatic => execute_call(op.as_instruction(), frame, ctx.module),
        OdinHeadCode::LoadLocal
        | OdinHeadCode::StoreLocal
        | OdinHeadCode::LoadArg
        | OdinHeadCode::LoadGlobal
        | OdinHeadCode::StoreGlobal
        | OdinHeadCode::CallImport
        | OdinHeadCode::CallIntrinsic => execute_control(op.as_instruction(), frame, ctx),
        OdinHeadCode::I32Add
        | OdinHeadCode::I32Sub
        | OdinHeadCode::I32Mul
        | OdinHeadCode::I32DivS
        | OdinHeadCode::I32RemS
        | OdinHeadCode::I32Eq
        | OdinHeadCode::I32Ne
        | OdinHeadCode::I32LtS
        | OdinHeadCode::I32LeS
        | OdinHeadCode::I32GtS
        | OdinHeadCode::I32GeS => execute_arithmetic(op.as_instruction(), frame, ctx.stack),
        OdinHeadCode::ObjectNew | OdinHeadCode::FieldGet | OdinHeadCode::FieldSet => execute_object(op.as_instruction(), frame, ctx),
    }
}
