use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::ffi::{FFIFunction, FFIResult, FFISignature, FFIType};

pub struct StdIoPrintln;
impl FFIFunction for StdIoPrintln {
    fn signature(&self) -> Option<FFISignature> {
        None
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
        vm.platform.stdout_write(&format!("{}\n", output));
        Ok(Value::null())
    }
}

pub struct StdIoPrint;
impl FFIFunction for StdIoPrint {
    fn signature(&self) -> Option<FFISignature> {
        None
    }
    fn call(&self, vm: &mut NyarVM, args: Vec<Value>) -> FFIResult {
        let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
        vm.platform.stdout_write(&output);
        vm.trace_log.lock().unwrap().push(output);
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
