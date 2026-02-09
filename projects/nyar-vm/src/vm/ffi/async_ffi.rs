use crate::vm::core::NyarVM;
use crate::vm::value::{Value, FutureStatus};
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
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

pub struct AsyncTimeout;
impl FFIFunction for AsyncTimeout {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any, FFIType::Int], // Future/Closure, ms
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let target = args.get(0).cloned().unwrap_or(Value::null());
        let ms = args.get(1).map(|v| v.as_int()).unwrap_or(0) as u64;
        
        let future = Value::future(&vm.gc);
        let future_clone = future;

        if target.is_future() {
            tokio::spawn(async move {
                 let timeout = tokio::time::sleep(Duration::from_millis(ms));
                
                tokio::select! {
                    _ = timeout => {
                        unsafe {
                            let f = future_clone.as_future_mut();
                            f.status = FutureStatus::Failed;
                            // Maybe set result to "Timeout"
                            if let Some(waker) = f.waker.take() {
                                waker.wake();
                            }
                        }
                    }
                    _ = async {
                        loop {
                            unsafe {
                                let f = target.as_future();
                                if f.status != FutureStatus::Pending {
                                    return f.status;
                                }
                            }
                            tokio::task::yield_now().await;
                        }
                    } => {
                        unsafe {
                            let f = future_clone.as_future_mut();
                            let target_f = target.as_future();
                            f.status = target_f.status;
                            f.result = target_f.result;
                            if let Some(waker) = f.waker.take() {
                                waker.wake();
                            }
                        }
                    }
                }
            });
        } else {
            return Err(vm.error(nyar_types::VmErrorKind::RuntimeError("Target must be a future".to_string())));
        }

        Ok(future)
    }
}

pub struct AsyncAwait;
impl FFIFunction for AsyncAwait {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if let Some(val) = args.get(0) {
            if val.is_closure() {
                let res = vm.call_closure_sync(*val, vec![])?;
                return Ok(res);
            } else if val.is_future() {
                let future = unsafe { val.as_future() };
                match future.status {
                    FutureStatus::Ready => {
                        return Ok(future.result);
                    }
                    FutureStatus::Failed => {
                        return Err(vm.error(nyar_types::VmErrorKind::FutureFailed));
                    }
                    FutureStatus::Pending => {
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
        Ok(Value::null())
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

        let mut new_vm = Box::new(vm.spawn_child());
        new_vm.push(closure).map_err(|e| vm.error(nyar_types::VmErrorKind::RuntimeError(e.to_string())))?;
        let vm_raw_ptr = Box::into_raw(new_vm);
        let vm_ptr = nyar_gc::ptr::SendPtr(std::ptr::NonNull::new(vm_raw_ptr).unwrap());
        let future = Value::future(&vm.gc);
        let future_clone = future;
        let gc_clone = vm.gc.clone();

        // Register the new VM as a global root while it is running in another thread.
        unsafe {
            gc_clone.register_global_root(vm_raw_ptr);
        }

        tokio::spawn(async move {
            let mut new_vm = unsafe { Box::from_raw(vm_ptr.as_ptr()) };
            let gc_clone = gc_clone;
            let future_clone = future_clone;
            // Although it is registered as a global root, we still use a StackRootGuard for consistency
            // if we were in a normal synchronous run. But here it is more about the persistent root.
            // Let's keep using PersistentRoot as well if it's already there, but register_global_root is the key.
            // let _root = unsafe { nyar_gc::PersistentRoot::new(gc_clone.clone(), &*new_vm) };

            // Manually setup the call frame for the closure
            if let Err(_e) = new_vm.execute_call_closure(0) {
                 unsafe {
                    let f = future_clone.as_future_mut();
                    f.status = FutureStatus::Failed;
                    if let Some(waker) = f.waker.take() {
                        waker.wake();
                    }
                }
                // Unregister before dropping
                unsafe {
                    gc_clone.unregister_global_root(&*new_vm);
                }
                return;
            }

            let vm_future = crate::vm::async_rt::VmFuture {
                vm: &mut new_vm,
                module_idx: 0, // Not used in poll_internal for execution, but good to have
                chunk_idx: 0,
            };

            let res = vm_future.await;

            // Unregister global root after execution finishes
            unsafe {
                gc_clone.unregister_global_root(&*new_vm);
            }

            match res {
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
