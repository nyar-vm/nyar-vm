use nyar_vm::vm::value::{Value, ValueTag};
use nyar_gc::NyarGc;

#[test]
fn test_nan_boxing_basic() {
    let v_int = Value::int(42);
    assert_eq!(v_int.tag(), ValueTag::Int);
    assert_eq!(v_int.as_int(), 42);

    let v_bool = Value::bool(true);
    assert_eq!(v_bool.tag(), ValueTag::Bool);
    assert!(v_bool.as_bool());

    let v_null = Value::null();
    assert_eq!(v_null.tag(), ValueTag::Null);

    let v_float = Value::float(3.14);
    assert!(v_float.is_float());
    assert!((v_float.as_float() - 3.14).abs() < 1e-10);
}

#[test]
fn test_nan_boxing_gc_types() {
    let gc = NyarGc::new();
    
    let s = "Hello, NaN-boxing!".to_string();
    let v_str = Value::string(s.clone(), &gc);
    assert_eq!(v_str.tag(), ValueTag::String);
    unsafe {
        assert_eq!(v_str.as_string(), &s);
    }

    let items = vec![Value::int(1), Value::int(2), Value::int(3)];
    let v_list = Value::list(items.clone(), &gc);
    assert_eq!(v_list.tag(), ValueTag::List);
    unsafe {
        let list = v_list.as_list();
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.items[0].as_int(), 1);
    }
}

#[test]
fn test_float_edge_cases() {
    // Test that we don't accidentally treat NaN as our internal boxed values
    let nan = f64::NAN;
    let v_nan = Value::float(nan);
    assert!(v_nan.is_float());
    assert!(v_nan.as_float().is_nan());

    let inf = f64::INFINITY;
    let v_inf = Value::float(inf);
    assert!(v_inf.is_float());
    assert_eq!(v_inf.as_float(), inf);
}
