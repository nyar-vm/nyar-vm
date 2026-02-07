use nyar_vm::NyarVM;
use nyar_vm::vm::value::Value;

#[test]
fn test_html_parse_and_select() {
    let mut vm = NyarVM::new();
    
    // 1. Test std.html.parse
    let html_content = r#"
        <html>
            <head><title>Test Page</title></head>
            <body>
                <h1 class="title">Hello World</h1>
                <a href="https://example.com">Link 1</a>
                <a href="/internal">Link 2</a>
            </body>
        </html>
    "#;
    
    let parse_func = vm.ffi.get("std.html.parse").expect("std.html.parse not registered");
    let args = vec![Value::string(html_content.to_string(), &vm.gc)];
    let doc_id_val = parse_func.call(&mut vm, args).expect("Failed to parse HTML");
    let doc_id = doc_id_val.as_int();
    assert!(doc_id > 0);

    // 2. Test std.html.select_text
    let select_text_func = vm.ffi.get("std.html.select_text").expect("std.html.select_text not registered");
    
    // Select title
    let args = vec![doc_id_val, Value::string("title".to_string(), &vm.gc)];
    let titles = select_text_func.call(&mut vm, args).expect("Failed to select title text");
    let title_list = titles.try_as_list().expect("Should be a list");
    assert_eq!(title_list.len(), 1);
    assert_eq!(title_list[0].try_as_str().unwrap(), "Test Page");

    // Select h1.title
    let args = vec![doc_id_val, Value::string("h1.title".to_string(), &vm.gc)];
    let h1_texts = select_text_func.call(&mut vm, args).expect("Failed to select h1 text");
    let h1_list = h1_texts.try_as_list().expect("Should be a list");
    assert_eq!(h1_list.len(), 1);
    assert_eq!(h1_list[0].try_as_str().unwrap(), "Hello World");

    // 3. Test std.html.select_attr
    let select_attr_func = vm.ffi.get("std.html.select_attr").expect("std.html.select_attr not registered");
    
    // Select href from all <a>
    let args = vec![
        doc_id_val, 
        Value::string("a".to_string(), &vm.gc),
        Value::string("href".to_string(), &vm.gc)
    ];
    let hrefs = select_attr_func.call(&mut vm, args).expect("Failed to select attributes");
    let href_list = hrefs.try_as_list().expect("Should be a list");
    assert_eq!(href_list.len(), 2);
    assert_eq!(href_list[0].try_as_str().unwrap(), "https://example.com");
    assert_eq!(href_list[1].try_as_str().unwrap(), "/internal");
}
