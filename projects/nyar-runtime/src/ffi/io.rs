use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;
use nyar_vm::vm::platform::NyarPlatform;
use crate::ffi::FFIResult;

pub fn std_io_println(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
    vm.platform.stdout_write(&format!("{}\n", output));
    Ok(Value::null())
}

pub fn std_io_print(vm: &mut NyarVM, args: &[Value]) -> FFIResult {
    let output = args.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(" ");
    vm.platform.stdout_write(&output);
    vm.trace_log.lock().unwrap().push(output);
    Ok(Value::null())
}

pub fn std_io_read_line(vm: &mut NyarVM, _args: &[Value]) -> FFIResult {
    let input = vm.platform.stdin_read_line();
    Ok(Value::string(input, &vm.gc))
}
