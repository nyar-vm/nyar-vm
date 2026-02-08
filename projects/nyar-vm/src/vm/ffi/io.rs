use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};

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
            vm.platform.stdout_write(&format!("{}\n", arg));
        } else {
            vm.platform.stdout_write("\n");
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
            vm.platform.stdout_write(&s);
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
        let input = vm.platform.stdin_read_line();
        Ok(Value::string(input, &vm.gc))
    }
}
