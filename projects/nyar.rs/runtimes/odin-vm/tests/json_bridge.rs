use nyar_gc::{ObjectHeap, Value};
use odin_vm::{
    error::OdinRuntimeError,
    json_bridge::{materialize_value_from_json, parse_call_args_json, value_from_json, value_to_json_with_heap},
};

#[test]
fn parse_args_json_scalars() {
    let args = parse_call_args_json("[1, 2, 3]").expect("parse");
    assert_eq!(args.len(), 3);
    assert!(matches!(args[0], Value::I32(1)));
}

#[test]
fn structured_json_requires_export_contract() {
    let mut heap = ObjectHeap::new();
    assert!(materialize_value_from_json(&serde_json::json!([3, 3]), Some(&mut heap)).is_err());
    for layout_id in [0, 1, 42] {
        let id = heap.alloc(nyar_gc::ObjectPayload::LayoutObject { layout_id, slots: vec![Value::I32(3), Value::I32(3)] });
        assert!(value_to_json_with_heap(&Value::Object(id), Some(&heap)).is_err());
    }
}

#[test]
fn json_object_requires_export_contract() {
    let result = value_from_json(&serde_json::json!({"value": 1}));
    assert!(matches!(result, Err(OdinRuntimeError::TypeMismatch { .. })));
}

#[test]
fn json_array_requires_heap() {
    let result = value_from_json(&serde_json::json!([1, 2]));
    assert!(matches!(result, Err(OdinRuntimeError::TypeMismatch { .. })));
}

fn echo_contract(ty: acorn_odin::OvmAbiType) -> acorn_odin::OvmAbiExport {
    acorn_odin::OvmAbiExport {
        logical_name: "echo".to_string(),
        params: vec![acorn_odin::OvmAbiParam { index: 0, name: "value".to_string(), ty: ty.clone(), carrier: "handle".to_string() }],
        returns: ty,
        returns_carrier: "handle".to_string(),
    }
}

#[test]
fn reference_instance_cannot_be_rebuilt_from_an_array_or_field_snapshot() {
    use acorn_odin::OvmAbiType;
    use odin_vm::json_bridge::{parse_call_args_json_with_contract, value_to_json_with_contract};

    let contract = echo_contract(OvmAbiType::InstanceHandle { layout: 0, name: "Measurement".to_string(), module_path: "demo".to_string() });
    let mut heap = ObjectHeap::new();
    for input in ["[[1, 2]]", r#"[{"samples":[1,2],"version":99}]"#] {
        let error = parse_call_args_json_with_contract(input, &contract, &mut heap).expect_err("引用实例不接受宿主重建");
        assert!(error.to_string().contains("instance handle construction contract"), "{error}");
    }
    let samples = heap.alloc_layout_object(1, 2);
    let instance = heap.alloc(nyar_gc::ObjectPayload::LayoutObject { layout_id: 0, slots: vec![Value::Object(samples), Value::I64(99)] });
    let error = value_to_json_with_contract(&Value::Object(instance), &contract.returns, &heap).expect_err("引用实例不允许序列化字段快照");
    assert!(error.to_string().contains("instance handle serialization contract"), "{error}");
}

#[test]
fn declared_arrays_roundtrip_elements_without_nominal_layout() {
    use acorn_odin::OvmAbiType;
    use odin_vm::json_bridge::{parse_call_args_json_with_contract, value_to_json_with_contract};

    let contract = echo_contract(OvmAbiType::Array { element: Box::new(OvmAbiType::Array { element: Box::new(OvmAbiType::I64) }) });
    let mut heap = ObjectHeap::new();
    let values = parse_call_args_json_with_contract("[[[1,2],[],[3]]]", &contract, &mut heap).expect("声明数组按元素合同物化");
    let output = value_to_json_with_contract(&values[0], &contract.returns, &heap).expect("声明数组往返");
    assert_eq!(output, serde_json::json!([[1, 2], [], [3]]));
}
