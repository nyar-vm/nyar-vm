use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_vm::vm::value::Value;
use nyar_types::NyarError;

pub struct StdTomlParse;

impl FFIFunction for StdTomlParse {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::String], ret: FFIType::Any })
    }

    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        Err(NyarError::RuntimeError("TOML parse not implemented yet".to_string()))
    }
}

pub struct StdTomlStringify;

impl FFIFunction for StdTomlStringify {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::Any], ret: FFIType::String })
    }

    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        Err(NyarError::RuntimeError("TOML stringify not implemented yet".to_string()))
    }
}
