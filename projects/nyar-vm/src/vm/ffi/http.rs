use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_types::NyarError;
use std::sync::{Arc, RwLock, OnceLock};
use reqwest::{Client, Proxy};
use tokio::runtime::Runtime;

static HTTP_CLIENT: OnceLock<Arc<RwLock<Client>>> = OnceLock::new();
static PROXY_URL: OnceLock<Arc<RwLock<Option<String>>>> = OnceLock::new();
static RUNTIME: OnceLock<Runtime> = OnceLock::new();

fn get_client() -> Arc<RwLock<Client>> {
    HTTP_CLIENT.get_or_init(|| Arc::new(RwLock::new(Client::new()))).clone()
}

fn get_proxy_url() -> Arc<RwLock<Option<String>>> {
    PROXY_URL.get_or_init(|| Arc::new(RwLock::new(None))).clone()
}

fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
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
        let url_str = url_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Url must be a string".to_string()))?;

        let client = get_client().read().unwrap().clone();
        let rt = get_runtime();
        
        let response = rt.block_on(async {
            client.get(url_str).send().await?.text().await
        }).map_err(|e| NyarError::RuntimeError(e.to_string()))?;

        Ok(Value::string(response, &vm.gc))
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
        let url_str = url_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Url must be a string".to_string()))?;
        let body_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing body argument".to_string()))?;
        let body_str = body_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Body must be a string".to_string()))?;

        let client = get_client().read().unwrap().clone();
        let rt = get_runtime();
        
        let response = rt.block_on(async {
            client.post(url_str).body(body_str.to_string()).send().await?.text().await
        }).map_err(|e| NyarError::RuntimeError(e.to_string()))?;

        Ok(Value::string(response, &vm.gc))
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
