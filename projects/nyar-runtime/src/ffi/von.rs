use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_vm::vm::value::Value;
use nyar_types::NyarError;

pub struct StdVonParse;

impl FFIFunction for StdVonParse {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::String], ret: FFIType::Any })
    }

    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        Err(NyarError::RuntimeError("VON parse not implemented yet".to_string()))
    }
}

pub struct StdVonStringify;

impl FFIFunction for StdVonStringify {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature { params: vec![FFIType::Any], ret: FFIType::String })
    }

    fn call(&self, _vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        Err(NyarError::RuntimeError("VON stringify not implemented yet".to_string()))
    }
}
