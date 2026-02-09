use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Future, FutureStatus};
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_gc::Root;
use nyar_types::NyarError;

pub struct StdFsReadToString;
impl FFIFunction for StdFsReadToString {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::String,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        
        let content = vm.platform.fs_read_to_string(path_str)?;
        Ok(Value::string(content, &vm.gc))
    }
}

pub struct AsyncFsReadToString;
impl FFIFunction for AsyncFsReadToString {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?.to_string();
        
        let future_val = Value::future(&vm.gc);
        let root: Root<Future> = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let gc = vm.gc.clone();

        tokio::spawn(async move {
            let res = tokio::fs::read_to_string(&path_str).await;
            unsafe {
                let f = root.as_mut();
                match res {
                    Ok(content) => {
                        f.status = FutureStatus::Ready;
                        f.result = Value::string(content, &gc);
                        root.as_gc().write_barrier(&gc);
                    }
                    Err(_e) => {
                        f.status = FutureStatus::Failed;
                        root.as_gc().write_barrier(&gc);
                    }
                }
                if let Some(waker) = f.waker.take() {
                    waker.wake();
                }
            }
        });

        Ok(future_val)
    }
}

pub struct StdFsWrite;
impl FFIFunction for StdFsWrite {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String, FFIType::String],
            ret: FFIType::Null,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        let content_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing content argument".to_string()))?;
        let content_str = content_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Content must be a string".to_string()))?;
        
        vm.platform.fs_write(path_str, content_str).map_err(NyarError::RuntimeError)?;
        Ok(Value::null())
    }
}

pub struct AsyncFsWrite;
impl FFIFunction for AsyncFsWrite {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String, FFIType::String],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?.to_string();
        let content_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing content argument".to_string()))?;
        let content_str = content_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Content must be a string".to_string()))?.to_string();
        
        let future_val = Value::future(&vm.gc);
        let root: Root<Future> = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let gc = vm.gc.clone();

        tokio::spawn(async move {
            let res = tokio::fs::write(&path_str, &content_str).await;
            unsafe {
                let f = root.as_mut();
                match res {
                    Ok(_) => {
                        f.status = FutureStatus::Ready;
                        f.result = Value::null();
                        root.as_gc().write_barrier(&gc);
                    }
                    Err(_e) => {
                        f.status = FutureStatus::Failed;
                        root.as_gc().write_barrier(&gc);
                    }
                }
                if let Some(waker) = f.waker.take() {
                    waker.wake();
                }
            }
        });

        Ok(future_val)
    }
}

pub struct StdFsExists;
impl FFIFunction for StdFsExists {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Bool,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        
        Ok(Value::bool(vm.platform.fs_exists(path_str)))
    }
}

pub struct StdFsRemoveFile;
impl FFIFunction for StdFsRemoveFile {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Null,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        
        vm.platform.fs_remove_file(path_str).map_err(NyarError::RuntimeError)?;
        Ok(Value::null())
    }
}
