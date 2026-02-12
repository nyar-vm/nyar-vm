use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Future, FutureStatus};
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use crate::net::NetworkHandle;
use nyar_gc::Root;
use nyar_types::NyarError;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct TcpConnect;
impl FFIFunction for TcpConnect {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let addr = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing address argument".to_string()))?
            .try_as_str().ok_or_else(|| NyarError::RuntimeError("Address must be a string".to_string()))?.to_string();
        let future_val = Value::future(&vm.gc);
        let root: Root<Future> = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let network = vm.network.clone();
        let gc = vm.gc.clone();
        
        tokio::spawn(async move {
            match tokio::net::TcpStream::connect(addr).await {
                Ok(stream) => {
                    let id = network.insert(NetworkHandle::TcpStream(stream));
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Ready;
                        f.result = Value::int(id as i64);
                        root.as_gc().write_barrier(&gc);
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
                Err(_) => {
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Failed;
                        root.as_gc().write_barrier(&gc);
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            }
        });
        
        Ok(future_val)
    }
}

pub struct TcpRead;
impl FFIFunction for TcpRead {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int, FFIType::Int],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing socket ID argument".to_string()))?.as_int() as u64;
        let len = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing length argument".to_string()))?.as_int() as usize;
        let future_val = Value::future(&vm.gc);
        let root: Root<Future> = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let network = vm.network.clone();
        let gc = vm.gc.clone();
        
        tokio::spawn(async move {
            if let Some(mut handle) = network.handles.get_mut(&id) {
                if let NetworkHandle::TcpStream(ref mut stream) = *handle {
                    let mut buf = vec![0u8; len];
                    match stream.read(&mut buf).await {
                        Ok(n) => {
                            buf.truncate(n);
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Ready;
                                f.result = Value::bytes(buf, &gc);
                                root.as_gc().write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                        Err(_) => {
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Failed;
                                root.as_gc().write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                    }
                } else {
                     unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Failed;
                        root.as_gc().write_barrier(&gc);
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            } else {
                unsafe {
                    let f = root.as_mut();
                    f.status = FutureStatus::Failed;
                    root.as_gc().write_barrier(&gc);
                    if let Some(waker) = f.waker.take() {
                        waker.wake();
                    }
                }
            }
        });
        
        Ok(future_val)
    }
}

pub struct TcpWrite;
impl FFIFunction for TcpWrite {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int, FFIType::Any],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).map(|v| v.as_int()).unwrap_or(-1) as u64;
        let data = args.get(1).cloned().unwrap_or(Value::null());
        
        let bytes = if data.is_bytes() {
            unsafe { data.as_bytes().data.clone() }
        } else if let Some(s) = data.try_as_str() {
            s.as_bytes().to_vec()
        } else {
            return Err(vm.error(nyar_types::VmErrorKind::RuntimeError("Invalid data type for TcpWrite".to_string())));
        };

        let future_val = Value::future(&vm.gc);
        let root = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let network = vm.network.clone();
        
        tokio::spawn(async move {
            if let Some(mut handle) = network.handles.get_mut(&id) {
                if let NetworkHandle::TcpStream(ref mut stream) = *handle {
                    match stream.write_all(&bytes).await {
                        Ok(_) => {
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Ready;
                                f.result = Value::null();
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                        Err(_) => {
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Failed;
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                    }
                } else {
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Failed;
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            } else {
                unsafe {
                    let f = root.as_mut();
                    f.status = FutureStatus::Failed;
                    if let Some(waker) = f.waker.take() {
                        waker.wake();
                    }
                }
            }
        });
        
        Ok(future_val)
    }
}

pub struct TcpListen;
impl FFIFunction for TcpListen {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let addr = args.get(0).and_then(|v| v.try_as_str()).unwrap_or("").to_string();
        let future_val = Value::future(&vm.gc);
        let root = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let network = vm.network.clone();
        
        tokio::spawn(async move {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(listener) => {
                    let id = network.insert(NetworkHandle::TcpListener(listener));
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Ready;
                        f.result = Value::int(id as i64);
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
                Err(_) => {
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Failed;
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            }
        });
        
        Ok(future_val)
    }
}

pub struct TcpAccept;
impl FFIFunction for TcpAccept {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int],
            ret: FFIType::Any,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).map(|v| v.as_int()).unwrap_or(-1) as u64;
        let future_val = Value::future(&vm.gc);
        let root = Root::new(vm.gc.clone(), unsafe { future_val.as_gc_future() });
        let network = vm.network.clone();
        
        tokio::spawn(async move {
            if let Some(mut handle) = network.handles.get_mut(&id) {
                if let NetworkHandle::TcpListener(ref mut listener) = *handle {
                    match listener.accept().await {
                        Ok((stream, _)) => {
                            let stream_id = network.insert(NetworkHandle::TcpStream(stream));
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Ready;
                                f.result = Value::int(stream_id as i64);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                        Err(_) => {
                            unsafe {
                                let f = root.as_mut();
                                f.status = FutureStatus::Failed;
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                    }
                } else {
                    unsafe {
                        let f = root.as_mut();
                        f.status = FutureStatus::Failed;
                        if let Some(waker) = f.waker.take() {
                            waker.wake();
                        }
                    }
                }
            } else {
                unsafe {
                    let f = root.as_mut();
                    f.status = FutureStatus::Failed;
                    if let Some(waker) = f.waker.take() {
                        waker.wake();
                    }
                }
            }
        });
        
        Ok(future_val)
    }
}

pub struct TcpClose;
impl FFIFunction for TcpClose {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Int],
            ret: FFIType::Null,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let id = args.get(0).map(|v| v.as_int()).unwrap_or(-1) as u64;
        vm.network.remove(id);
        Ok(Value::null())
    }
}
