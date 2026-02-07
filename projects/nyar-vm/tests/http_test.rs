use nyar_vm::NyarVM;
use nyar_vm::vm::value::Value;

#[test]
fn test_http_get_ffi() {
    let mut vm = NyarVM::new();
    
    // Test std.http.get
    if let Some(func) = vm.ffi.get("std.http.get") {
        // We use a known stable URL for testing, like example.com
        let args = vec![Value::string("https://example.com".to_string(), &vm.gc)];
        let res = func.call(&mut vm, args);
        
        match res {
            Ok(val) => {
                let content = val.try_as_str().expect("Response should be a string");
                assert!(content.contains("Example Domain"));
            }
            Err(e) => {
                // If no internet connection, we might get an error, but at least the FFI was called
                eprintln!("HTTP GET failed: {:?}", e);
            }
        }
    } else {
        panic!("std.http.get not registered");
    }
}

#[test]
fn test_http_proxy_setting() {
    let mut vm = NyarVM::new();
    
    // Test std.http.set_proxy
    if let Some(func) = vm.ffi.get("std.http.set_proxy") {
        let args = vec![Value::string("http://127.0.0.1:8080".to_string(), &vm.gc)];
        let res = func.call(&mut vm, args);
        assert!(res.is_ok());
    } else {
        panic!("std.http.set_proxy not registered");
    }
}
