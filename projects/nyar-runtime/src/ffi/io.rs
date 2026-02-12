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
    let mut input = String::new();
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| {
            let _ = std::io::stdin().read_line(&mut input);
        });
    } else {
        let _ = std::io::stdin().read_line(&mut input);
    }
    let input = input.trim_end().to_string();
    Ok(Value::string(input, &vm.gc))
}
