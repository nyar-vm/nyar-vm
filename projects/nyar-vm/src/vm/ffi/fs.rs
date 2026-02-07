use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_types::NyarError;
use std::fs;
use std::path::Path;

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
        
        let content = fs::read_to_string(path_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
        Ok(Value::string(content, &vm.gc))
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        let content_val = args.get(1).ok_or_else(|| NyarError::RuntimeError("Missing content argument".to_string()))?;
        let content_str = content_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Content must be a string".to_string()))?;
        
        fs::write(path_str, content_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
        Ok(Value::null())
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        
        Ok(Value::bool(Path::new(path_str).exists()))
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
    fn call(&self, _vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let path_val = args.get(0).ok_or_else(|| NyarError::RuntimeError("Missing path argument".to_string()))?;
        let path_str = path_val.try_as_str().ok_or_else(|| NyarError::RuntimeError("Path must be a string".to_string()))?;
        
        fs::remove_file(path_str).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
        Ok(Value::null())
    }
}
