use crate::vm::value::Value;
use nyar_types::{EffectInfo, NyarError};

#[derive(Clone)]
pub struct HandlerFrame {
    pub module_idx: usize,
    pub catch_chunk: usize,
    pub frame_depth: usize,
}

pub trait EffectHandler: Send + Sync {
    fn perform_effect(
        &self,
        vm: &mut crate::vm::core::NyarVM,
        module_idx: usize,
        effect: EffectInfo,
        args: Vec<Value>,
    ) -> Result<Option<Value>, NyarError>;
}

pub fn perform_effect_internal(
    vm: &mut crate::vm::core::NyarVM,
    module_idx: usize,
    effect: EffectInfo,
    args: Vec<Value>,
) -> Result<Option<Value>, NyarError> {
    // 1. Try external handler first
    if let Some(handler) = vm.effect_handler.clone() {
        match handler.perform_effect(vm, module_idx, effect.clone(), args.clone()) {
            Ok(res) => return Ok(res),
            Err(e) if matches!(*e.kind, nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::UnhandledEffect(_))) => {
                // Continue to intrinsic handlers
            }
            Err(e) => return Err(e),
        }
    }

    // 2. Intrinsic handlers (Mechanism)
    if effect.name.parts.len() == 1 {
        let name = &effect.name.parts[0];
        match name.as_str() {
            "await" => {
                if let Some(val) = args.get(0) {
                    if val.is_closure() {
                        let res = vm.call_closure_sync(*val, vec![])?;
                        return Ok(Some(res));
                    } else if val.is_future() {
                        let future = val.try_as_future().unwrap();
                        match future.status {
                            crate::vm::value::FutureStatus::Ready => {
                                return Ok(Some(future.result));
                            }
                            crate::vm::value::FutureStatus::Failed => {
                                return Err(vm.error(nyar_types::VmErrorKind::RuntimeError("Future failed".to_string())));
                            }
                            crate::vm::value::FutureStatus::Pending => {
                                // Register waker and yield
                                unsafe {
                                    let future_mut = val.as_future_mut();
                                    future_mut.waker = vm.current_waker.clone();
                                }
                                return Err(vm.error(nyar_types::VmErrorKind::YieldAsync));
                            }
                        }
                    }
                }
                return Ok(None);
            }
            _ => {}
        }
    }

    // Handle Token variants using QualifiedName (Mechanism for Algebraic Effects)
    if effect.name.parts.len() >= 2 && effect.name.parts[effect.name.parts.len() - 2] == "Token" {
        let variant_name = effect.name.parts.last().map(|s| s.as_str()).unwrap_or("");
        // Find Token class
        let class_idx = {
            let module = vm.get_module(module_idx);
            module
                .classes
                .iter()
                .position(|c| c.name.parts.last().map(|s| s.as_str()) == Some("Token"))
        };
        if let Some(idx) = class_idx {
            let idx = idx as u16;
            let val = args.get(0).cloned().unwrap_or(Value::null());
            // Token enum has __variant__ and one field (u or x) mapped to _0
            let fields = vec![Value::string(variant_name.to_string(), &vm.gc), val];
            let obj = Value::object(module_idx, idx, fields, &vm.gc);
            return Ok(Some(obj));
        }
    }

    // 3. Runtime Fallback (Mechanism for standard libraries)
    let effect_name = effect.name.to_string();
    if let Some(func) = vm.runtime.resolve(&effect_name) {
        let res = func(vm, &args)?;
        return Ok(Some(res));
    }

    vm.log("Traceback (most recent call last):");
    vm.log(&format!("UnhandledEffect: {} at source {} offset {}", effect.name, effect.location.source_id, effect.location.offset));
    Err(vm.error(nyar_types::VmErrorKind::UnhandledEffect(effect.name)))
}

