use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::{Value, FutureStatus};
use crate::ffi::FFIResult;
use nyar_gc::{Root, Trace};
use nyar_types::NyarError;
use std::path::Path;

pub fn std_fs_read_to_string(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
    
    let content = std::fs::read_to_string(path_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
    Ok(Value::string(content, &vm.gc))
}

pub fn async_fs_read_to_string(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    // We need to clone the string because we move it into the async block
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?.to_string();
    
    let future_val = Value::future(&vm.gc);
    // SAFETY: We are creating a Root for the Future object to prevent GC while it's being processed asynchronously
    let root = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
    let gc = vm.gc.clone();

    tokio::spawn(async move {
        let res = tokio::fs::read_to_string(&path_str).await;
        // We need to acquire the lock/access safely. 
        // Note: In a real multi-threaded GC, we need proper synchronization.
        // Here we assume we can write to the future object if we hold the Root.
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

pub fn std_fs_write(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
    let content_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing content argument".to_string()))?;
    let content_str = content_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Content must be a string".to_string()))?;
    
    std::fs::write(path_str, content_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
    Ok(Value::null())
}

pub fn async_fs_write(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?.to_string();
    let content_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing content argument".to_string()))?;
    let content_str = content_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Content must be a string".to_string()))?.to_string();
    
    let future_val = Value::future(&vm.gc);
    let root = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
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

pub fn std_fs_exists(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
    
    Ok(Value::bool(Path::new(path_str).exists()))
}

pub fn std_fs_remove_file(_vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
    let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
    
    std::fs::remove_file(path_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
    Ok(Value::null())
}
