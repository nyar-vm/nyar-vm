use nyar_vm::NyarVM;
use nyar_vm::vm::value::Value;

#[test]
fn test_json_parse_ffi() {
    let mut vm = NyarVM::new();
    
    if let Some(func) = vm.ffi.get("std.config.json.Json.parse") {
        let json_str = r#"{"name": "Valkyrie", "version": 1, "active": true, "tags": ["lang", "vm"]}"#;
        let args = vec![Value::string(json_str.to_string(), &vm.gc)];
        let res = func.call(&mut vm, args).expect("JSON parse failed");
        
        unsafe {
            let obj = res.as_dyn_object();
            assert_eq!(obj.entries.get("name").unwrap().try_as_str().unwrap(), "Valkyrie");
            assert_eq!(obj.entries.get("version").unwrap().as_int(), 1);
            assert_eq!(obj.entries.get("active").unwrap().as_bool(), true);
            let tags = obj.entries.get("tags").unwrap().as_list();
            assert_eq!(tags.items.len(), 2);
            assert_eq!(tags.items[0].try_as_str().unwrap(), "lang");
            assert_eq!(tags.items[1].try_as_str().unwrap(), "vm");
        }
    } else {
        panic!("std.json.parse not registered");
    }
}

#[test]
fn test_json_parse_null() {
    let mut vm = NyarVM::new();
    if let Some(func) = vm.ffi.get("std.config.json.Json.parse") {
        let args = vec![Value::string("null".to_string(), &vm.gc)];
        let res = func.call(&mut vm, args).expect("JSON parse failed");
        assert!(res.is_null());
    }
}

#[test]
fn test_json_stringify_ffi() {
    let mut vm = NyarVM::new();
    if let Some(func) = vm.ffi.get("std.config.json.Json.stringify") {
        // Test primitive
        let args = vec![Value::int(42)];
        let res = func.call(&mut vm, args).expect("JSON stringify failed");
        assert_eq!(res.try_as_str().unwrap(), "42");

        // Test list
        let items = vec![Value::int(1), Value::bool(true), Value::string("hi".to_string(), &vm.gc)];
        let list = Value::list(items, &vm.gc);
        let args = vec![list];
        let res = func.call(&mut vm, args).expect("JSON stringify failed");
        assert_eq!(res.try_as_str().unwrap(), "[1,true,\"hi\"]");

        // Test object
        let obj = Value::dyn_object(&vm.gc);
        unsafe {
            obj.as_dyn_object_mut().entries.insert("key".to_string(), Value::string("value".to_string(), &vm.gc));
        }
        let args = vec![obj];
        let res = func.call(&mut vm, args).expect("JSON stringify failed");
        assert_eq!(res.try_as_str().unwrap(), "{\"key\":\"value\"}");
    } else {
        panic!("std.json.stringify not registered");
    }
}
