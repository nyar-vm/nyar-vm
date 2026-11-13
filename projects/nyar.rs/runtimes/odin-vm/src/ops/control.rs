use acorn_odin::{IntrinsicOpcode, OdinHeadCode, OdinInstruction};

use crate::{
    array_runtime::{array_get, array_length, array_push, array_set},
    error::OdinRuntimeError,
    frame::Frame,
    host::{ResolvedImport, execute_host_op},
    module::LoadedModule,
    ops::{ExecutionContext, StepResult},
    value::Value,
};

/// Dispatches control-flow and variable instructions.
pub fn execute_control(
    instruction: OdinInstruction,
    frame: &mut Frame,
    ctx: &mut ExecutionContext<'_>,
) -> Result<StepResult, OdinRuntimeError> {
    match instruction.code {
        OdinHeadCode::Jump => {
            frame.ip = frame.ip.wrapping_add(instruction.operand1 as usize);
            Ok(StepResult::Continue)
        }
        OdinHeadCode::JumpIfTrue | OdinHeadCode::JumpIfFalse => {
            let condition = ctx.stack.pop()?.to_bool();
            let should_jump = match instruction.code {
                OdinHeadCode::JumpIfTrue => condition,
                OdinHeadCode::JumpIfFalse => !condition,
                _ => false,
            };
            if should_jump {
                frame.ip = frame.ip.wrapping_add(instruction.operand1 as usize);
            }
            else {
                frame.ip += instruction.size as usize;
            }
            Ok(StepResult::Continue)
        }
        OdinHeadCode::LoadLocal | OdinHeadCode::LoadArg => {
            let index = instruction.operand1 as usize;
            let value = frame.locals.get(index).cloned().ok_or(OdinRuntimeError::LocalIndexOutOfRange(instruction.operand1))?;
            ctx.stack.push(value);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::StoreLocal => {
            let index = instruction.operand1 as usize;
            let value = ctx.stack.pop()?;
            let slot = frame.locals.get_mut(index).ok_or(OdinRuntimeError::LocalIndexOutOfRange(instruction.operand1))?;
            *slot = value;
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::LoadGlobal => {
            let index = instruction.operand1 as usize;
            let value = ctx.globals.get(index).cloned().ok_or(OdinRuntimeError::GlobalIndexOutOfRange(instruction.operand1))?;
            ctx.stack.push(value);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::StoreGlobal => {
            let index = instruction.operand1 as usize;
            let value = ctx.stack.pop()?;
            let slot = ctx.globals.get_mut(index).ok_or(OdinRuntimeError::GlobalIndexOutOfRange(instruction.operand1))?;
            let satb = ctx.heap.concurrent_mark().requires_satb();
            nyar_gc::write_value_slot(ctx.heap.barrier_mut(), slot, value, satb);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::CallImport => {
            let import_index = instruction.operand1;
            if import_index < 0 || (import_index as usize) >= ctx.module.resolved_imports.len() {
                return Err(OdinRuntimeError::ImportIndexOutOfRange(import_index));
            }
            let resolved = ctx.module.resolved_imports[import_index as usize];
            let arg_count = instruction.operand2.max(0) as usize;
            let mut args = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                args.push(ctx.stack.pop()?);
            }
            args.reverse();

            let result = match resolved {
                ResolvedImport::Host(op) => execute_host_op(op, &args, ctx.heap)?,
                ResolvedImport::External => {
                    return Err(OdinRuntimeError::UnsupportedFeature(
                        "non-host CallImport requires import-index binding; string host dispatch removed",
                    ));
                }
            };
            ctx.stack.push(result);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        OdinHeadCode::CallIntrinsic => {
            // 指令身份由 `acorn_odin::IntrinsicOpcode`（OVM 私有格式合同）给出；
            // 未知操作数 fail-closed，不得猜测或回落到语言 builtin 表。
            let intrinsic_index = instruction.operand1;
            let arg_count = instruction.operand2.max(0) as usize;
            let mut args = Vec::with_capacity(arg_count);
            for _ in 0..arg_count {
                args.push(ctx.stack.pop()?);
            }
            args.reverse();

            let intrinsic = IntrinsicOpcode::from_operand(intrinsic_index).map_err(|_| OdinRuntimeError::UnknownIntrinsic(intrinsic_index))?;

            let result = match intrinsic {
                IntrinsicOpcode::ArrayPush => array_push(ctx.heap, args.first().unwrap_or(&Value::Null), args.get(1).unwrap_or(&Value::Null))?,
                IntrinsicOpcode::ArrayLen => array_length(ctx.heap, args.first().unwrap_or(&Value::Null))?,
                IntrinsicOpcode::ArrayGet => array_get(ctx.heap, args.first().unwrap_or(&Value::Null), args.get(1).unwrap_or(&Value::Null))?,
                IntrinsicOpcode::ArraySet => array_set(
                    ctx.heap,
                    args.first().unwrap_or(&Value::Null),
                    args.get(1).unwrap_or(&Value::Null),
                    args.get(2).unwrap_or(&Value::Null),
                )?,
                IntrinsicOpcode::RefDeref => {
                    return Err(OdinRuntimeError::UnsupportedFeature("CallIntrinsic RefDeref"));
                }
                IntrinsicOpcode::IsNull => {
                    let value = args.first().unwrap_or(&Value::Null);
                    Value::Bool(matches!(value, Value::Null))
                }
                IntrinsicOpcode::UnwrapNull => match args.first().unwrap_or(&Value::Null) {
                    Value::Null => {
                        return Err(OdinRuntimeError::TypeMismatch { expected: "non-null value", actual: "null".to_string() });
                    }
                    other => other.clone(),
                },
            };
            ctx.stack.push(result);
            frame.ip += instruction.size as usize;
            Ok(StepResult::Continue)
        }
        _ => Err(OdinRuntimeError::UnknownOpcode(instruction.code as u8)),
    }
}

/// Enters a nested function call.
pub fn execute_call(instruction: OdinInstruction, frame: &mut Frame, module: &LoadedModule) -> Result<StepResult, OdinRuntimeError> {
    let function_index = instruction.operand1;
    if function_index < 0 {
        return Err(OdinRuntimeError::FunctionIndexOutOfRange(function_index));
    }
    let function_index = function_index as usize;
    if function_index >= module.functions.len() {
        return Err(OdinRuntimeError::FunctionIndexOutOfRange(function_index as i32));
    }
    frame.ip += instruction.size as usize;
    Ok(StepResult::Call { function_index })
}

/// Enters a nested call whose target comes from the operand stack.
///
/// 栈布局 `[arg0, …, argN-1, env, callee]`：只弹 `callee`（`i32` 函数索引），实参与
/// 环境槽留在栈上由新帧按 `arity` 消费（与 [`execute_call`] 的直调约定一致）。
/// `argc`（operand1）= 可见实参 + 1（闭包载体尾环境槽）：合成 lambda 的 `arity`
/// 含环境形参，具名函数引用的 `arity` 只有可见形参，后者的尾槽必须是 `null` 且
/// 在进入帧前消费掉。索引越界、argc/arity 不合合同或 callee 不是 `i32` 一律
/// 结构化失败，不回落、不猜测。
pub fn execute_call_indirect(
    instruction: OdinInstruction,
    frame: &mut Frame,
    ctx: &mut ExecutionContext<'_>,
) -> Result<StepResult, OdinRuntimeError> {
    let callee = ctx.stack.pop()?;
    let function_index = match callee {
        Value::I32(index) => index,
        other => {
            return Err(OdinRuntimeError::TypeMismatch { expected: "function index (i32)", actual: other.type_name().to_string() });
        }
    };
    if function_index < 0 || (function_index as usize) >= ctx.module.functions.len() {
        return Err(OdinRuntimeError::FunctionIndexOutOfRange(function_index));
    }
    let function_index = function_index as usize;
    let argc = instruction.operand1.max(0) as usize;
    let arity = ctx.module.functions[function_index].arity.max(0) as usize;
    if argc == arity + 1 {
        // 具名函数引用携带统一闭包载体，但目标 arity 不含环境形参：尾槽只能是
        // 显式 null，捕获环境不允许走这条直调路径。
        let environment = ctx.stack.pop()?;
        if !matches!(environment, Value::Null) {
            return Err(OdinRuntimeError::TypeMismatch {
                expected: "null capture environment for named function reference",
                actual: environment.type_name().to_string(),
            });
        }
    }
    else if argc != arity {
        return Err(OdinRuntimeError::ModuleLoad(format!(
            "CallIndirect argc {argc} does not match function[{function_index}] `{}` arity {arity}",
            ctx.module.functions[function_index].name
        )));
    }
    frame.ip += instruction.size as usize;
    Ok(StepResult::Call { function_index })
}
