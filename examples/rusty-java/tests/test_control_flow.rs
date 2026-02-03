use rusty_java::NyarFrontend;
use nyar_types::NyarFrontend as _;

#[test]
fn test_control_flow_conversion() {
    let source = include_str!("control_flow.java");
    let frontend = NyarFrontend::new();
    
    // 1. 解析
    let ast = frontend.parse(source).expect("Failed to parse Java");
    
    // 2. 转换为 UIR
    let uir = frontend.lower(&ast).expect("Failed to lower to UIR");
    
    // 3. 打印 UIR (调试用)
    println!("UIR: {:?}", uir);
    
    // 4. 验证 UIR 结构
    // 检查 UIR 是否包含 expected 的控制流扩展
    let uir_str = format!("{:?}", uir);
    assert!(uir_str.contains("\"if\""));
    assert!(uir_str.contains("\"while\""));
    assert!(uir_str.contains("\"do_while\""));
    assert!(uir_str.contains("\"for\""));
    assert!(uir_str.contains("\"switch\""));
    assert!(uir_str.contains("\"break\""));
    assert!(uir_str.contains("\"continue\""));
}
