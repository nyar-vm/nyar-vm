use rusty_java::MiniJavaFrontend;
use nyar_types::NyarFrontend;

#[test]
fn test_ops_conversion() {
    let source = include_str!("ops_test.java");
    let frontend = MiniJavaFrontend::default();
    
    // 1. 解析
    let ast = frontend.parse(source).expect("Failed to parse Java");
    
    // 2. 转换为 UIR
    let vfs = oak_vfs::MemoryVfs::new();
    let uir = frontend.lower(&ast, &vfs).expect("Failed to lower to UIR");
    
    // 3. 打印 UIR (调试用)
    println!("UIR: {:?}", uir);
    
    // 4. 验证 UIR 结构
    let uir_str = format!("{:?}", uir);
    
    // 算术运算符
    assert!(uir_str.contains("\"add\""));
    assert!(uir_str.contains("\"sub\""));
    assert!(uir_str.contains("\"mul\""));
    assert!(uir_str.contains("\"div\""));
    assert!(uir_str.contains("\"rem\""));
    
    // 位运算符
    assert!(uir_str.contains("\"and\""));
    assert!(uir_str.contains("\"or\""));
    assert!(uir_str.contains("\"xor\""));
    assert!(uir_str.contains("\"not\""));
    assert!(uir_str.contains("\"shl\""));
    assert!(uir_str.contains("\"shr\""));
    assert!(uir_str.contains("\"ushr\""));
    
    // 比较运算符
    assert!(uir_str.contains("\"eq\""));
    assert!(uir_str.contains("\"ne\""));
    assert!(uir_str.contains("\"lt\""));
    assert!(uir_str.contains("\"le\""));
    assert!(uir_str.contains("\"gt\""));
    assert!(uir_str.contains("\"ge\""));
    
    // 逻辑运算符
    assert!(uir_str.contains("\"land\""));
    assert!(uir_str.contains("\"lor\""));
    assert!(uir_str.contains("\"lnot\""));
    
    // 自增自减 (前缀和后缀)
    // 注意：在前缀模式下，我们生成了 add/sub 和 assign
    // 在后缀模式下，我们生成了 tmp 变量、add/sub 和 assign
}
