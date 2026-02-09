use crate::vm::core::NyarVM;
use crate::vm::value::{Value, Future, FutureStatus};
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_types::NyarError;
use std::sync::{Arc, RwLock, OnceLock};
use reqwest::{Client, Proxy};

static HTTP_CLIENT: OnceLock<Arc<RwLock<Client>>> = OnceLock::new();
static PROXY_URL: OnceLock<Arc<RwLock<Option<String>>>> = OnceLock::new();

fn get_client() -> Arc<RwLock<Client>> {
    HTTP_CLIENT.get_or_init(|| Arc::new(RwLock::new(Client::new()))).clone()
}

fn get_proxy_url() -> Arc<RwLock<Option<String>>> {
    PROXY_URL.get_or_init(|| Arc::new(RwLock::new(None))).clone()
}

pub struct StdHttpGet;
impl FFIFunction for StdHttpGet {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::String,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let url_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing url argument".to_string()))?;
        let url_str = url_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Url must be a string".to_string()))?.to_string();

        let future_val = Value::future(&vm.gc);
        let client = get_client().read().unwrap().clone();
        let gc = vm.gc.clone();

        tokio::spawn(async move {
            let res = client.get(&url_str).send().await;
            match res {
                Ok(resp) => {
                    let text_res = resp.text().await;
                    match text_res {
                        Ok(t) => {
                            unsafe {
                                let f = future_val.as_future_mut();
                                f.result = Value::string(t, &gc);
                                f.status = FutureStatus::Ready;
                                future_val.write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                        Err(_) => {
                            unsafe {
                                let f = future_val.as_future_mut();
                                f.status = FutureStatus::Failed;
                                future_val.write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    unsafe {
                        let f = future_val.as_future_mut();
                        f.status = FutureStatus::Failed;
                        future_val.write_barrier(&gc);
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

pub struct StdHttpPost;
impl FFIFunction for StdHttpPost {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String, FFIType::String],
            ret: FFIType::String,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let url_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing url argument".to_string()))?;
        let url_str = url_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Url must be a string".to_string()))?.to_string();
        let body_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing body argument".to_string()))?;
        let body_str = body_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Body must be a string".to_string()))?.to_string();

        let future_val = Value::future(&vm.gc);
        let client = get_client().read().unwrap().clone();
        let gc = vm.gc.clone();

        tokio::spawn(async move {
            let res = client.post(&url_str).body(body_str).send().await;
            match res {
                Ok(resp) => {
                    let text_res = resp.text().await;
                    match text_res {
                        Ok(t) => {
                            unsafe {
                                let f = future_val.as_future_mut();
                                f.result = Value::string(t, &gc);
                                f.status = FutureStatus::Ready;
                                future_val.write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                        Err(_) => {
                            unsafe {
                                let f = future_val.as_future_mut();
                                f.status = FutureStatus::Failed;
                                future_val.write_barrier(&gc);
                                if let Some(waker) = f.waker.take() {
                                    waker.wake();
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    unsafe {
                        let f = future_val.as_future_mut();
                        f.status = FutureStatus::Failed;
                        future_val.write_barrier(&gc);
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

pub struct StdHttpSetProxy;
impl FFIFunction for StdHttpSetProxy {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::String],
            ret: FFIType::Null,
        })
    }
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let proxy_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing proxy argument".to_string()))?;
        let proxy_str = proxy_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Proxy must be a string".to_string()))?;

        let lock = get_proxy_url();
        let mut proxy_url = lock.write().unwrap();
        *proxy_url = Some(proxy_str.to_string());

        let lock = get_client();
        let mut client = lock.write().unwrap();
        let new_client = Client::builder()
            .proxy(Proxy::all(proxy_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?)
            .build()
            .map_err(|e| NyarError::RuntimeError(e.to_string()))?;
        *client = new_client;

        Ok(Value::null())
    }
}
