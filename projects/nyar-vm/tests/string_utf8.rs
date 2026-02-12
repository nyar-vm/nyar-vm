use nyar_types::{NyarErrorKind, VmErrorKind};
use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;

fn assert_vm_runtime_error(err: nyar_types::NyarError) {
    match *err.kind {
        NyarErrorKind::Vm(VmErrorKind::RuntimeError(_)) => {}
        other => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn string_len_bytes_and_chars_are_utf8_aware() {
    let mut vm = NyarVM::new();
    let s = Value::string("é".to_string(), &vm.gc);

    vm.push(s).unwrap();
    vm.execute_string_len_bytes().unwrap();
    let bytes_len = vm.pop().unwrap().as_int();
    assert_eq!(bytes_len, 2);

    let s = Value::string("é".to_string(), &vm.gc);
    vm.push(s).unwrap();
    vm.execute_string_len_chars().unwrap();
    let chars_len = vm.pop().unwrap().as_int();
    assert_eq!(chars_len, 1);
}

#[test]
fn string_substr_rejects_non_utf8_boundary_indices() {
    let mut vm = NyarVM::new();
    let s = Value::string("é".to_string(), &vm.gc);

    vm.push(s).unwrap();
    vm.push(Value::int(0)).unwrap();
    vm.push(Value::int(1)).unwrap();
    let err = vm.execute_string_substr().unwrap_err();
    assert_vm_runtime_error(err);
}

#[test]
fn string_substr_allows_utf8_boundary_indices() {
    let mut vm = NyarVM::new();
    let s = Value::string("é".to_string(), &vm.gc);

    vm.push(s).unwrap();
    vm.push(Value::int(0)).unwrap();
    vm.push(Value::int(2)).unwrap();
    vm.execute_string_substr().unwrap();
    let out = vm.pop().unwrap();
    assert_eq!(out.try_as_str().unwrap(), "é");
}

#[test]
fn string_substr_rejects_negative_indices() {
    let mut vm = NyarVM::new();
    let s = Value::string("abc".to_string(), &vm.gc);

    vm.push(s).unwrap();
    vm.push(Value::int(-1)).unwrap();
    vm.push(Value::int(1)).unwrap();
    let err = vm.execute_string_substr().unwrap_err();
    assert_vm_runtime_error(err);
}

