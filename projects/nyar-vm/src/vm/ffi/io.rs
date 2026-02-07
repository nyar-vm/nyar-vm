use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};
use nyar_types::NyarError;
use std::io::{self, Write};

pub struct StdIoPrintln;
impl FFIFunction for StdIoPrintln {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any],
            ret: FFIType::Null,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if let Some(arg) = args.get(0) {
            vm.log(&format!("{}", arg));
        } else {
            vm.log("");
        }
        Ok(Value::null())
    }
}

pub struct StdIoPrint;
impl FFIFunction for StdIoPrint {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![FFIType::Any],
            ret: FFIType::Null,
        })
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        if let Some(arg) = args.get(0) {
            let s = format!("{}", arg);
            if let Some(cb) = &vm.stdout {
                cb(&s);
            } else {
                print!("{}", s);
                let _ = io::stdout().flush();
            }
            vm.trace_log.lock().unwrap().push(s);
        }
        Ok(Value::null())
    }
}

pub struct StdIoReadLine;
impl FFIFunction for StdIoReadLine {
    fn signature(&self) -> Option<FFISignature> {
        Some(FFISignature {
            params: vec![],
            ret: FFIType::String,
        })
    }
    fn call(&self, vm: &mut NyarVM, _args: Vec<Value>) -> FFIResult {
        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(|e| NyarError::RuntimeError(e.to_string()))?;
        Ok(Value::string(input.trim_end().to_string(), &vm.gc))
    }
}
