use crate::vm::core::NyarVM;
use crate::vm::value::{Value, FutureStatus};
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use std::sync::Arc;
use std::time::Duration;

pub struct AsyncDelay;
impl FFIFunction for AsyncDelay {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let ms = args.get(0).map(|v| v.as_int()).unwrap_or(0) as u64;
        let future = Value::future(&vm.gc);
        let future_clone = future;
        
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(ms)).await;
            unsafe {
                let f = future_clone.as_future_mut();
                f.status = FutureStatus::Ready;
                f.result = Value::null();
                if let Some(waker) = f.waker.take() {
                    waker.wake();
                }
            }
        });
        
        Ok(future)
    }
}

pub struct AsyncSpawn;
impl FFIFunction for AsyncSpawn {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any], // Closure
            ret: FFIType::Any, // Future
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let closure = args.get(0).cloned().unwrap_or(Value::null());
        if !closure.is_closure() {
            return Err(vm.error(nyar_types::VmErrorKind::RuntimeError("Invalid closure".to_string())));
        }

        let mut new_vm = NyarVM::new();
        new_vm.gc = vm.gc.clone();
        new_vm.modules = vm.modules.clone();
        new_vm.module_names = vm.module_names.clone();
        new_vm.trace_log = vm.trace_log.clone();
        new_vm.ffi = vm.ffi.clone();
        new_vm.symbol_table = vm.symbol_table.clone();
        new_vm.builtins = vm.builtins.clone();
        new_vm.jit = vm.jit.clone();

        let future = Value::future(&vm.gc);
        let future_clone = future;

        // Setup the new VM to call the closure
        new_vm.push(closure).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
        // We need to trigger the call. 
        // A simple way is to use a specialized entry point or just manually setup the frame.
        // For now, let's assume the closure is already pushed and we just need to execute.
        
        tokio::spawn(async move {
            // Manually setup the call frame for the closure
            if let Err(e) = new_vm.execute_call_closure(0) {
                 unsafe {
                    let f = future_clone.as_future_mut();
                    f.status = FutureStatus::Failed;
                    if let Some(waker) = f.waker.take() {
                        waker.wake();
                    }
                }
                return;
            }

            let vm_future = crate::vm::async_rt::VmFuture {
                vm: &mut new_vm,
                module_idx: 0, // Not used in poll_internal for execution, but good to have
                chunk_idx: 0,
            };

            match vm_future.await {
                Ok(res) => {
                    unsafe {
                        let f = future_clone.as_future_mut();
                        f.status = FutureStatus::Ready;
                        f.result = res;
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
                Err(_) => {
                    unsafe {
                        let f = future_clone.as_future_mut();
                        f.status = FutureStatus::Failed;
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            }
        });

        Ok(future)
    }
}
