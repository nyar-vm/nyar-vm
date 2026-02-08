use oak_core::Lexer;
use oak_lua::{lexer::LuaLexer, language::LuaLanguage};

#[test]
pub fn test_lexer_functionality() {
    println!("开始测试 Lua 词法分析器...");

    let language = LuaLanguage {};
    // 测试简单的赋值语句
    let input = "x = 42";
    let lexer = LuaLexer::new(&language);
    let mut session = oak_core::parser::ParseSession::<LuaLanguage>::default();
    lexer.lex(input, &[], &mut session);

    println!("✓ 词法分析成功");
    println!("  输入: {}", input);
    let tokens_vec = session.tokens.get_ref();
    println!("  生成的 token 数量: {}", tokens_vec.len());
    assert!(tokens_vec.len() >= 3);

    // 打印前几个 token
    for (i, token) in tokens_vec.iter().take(5).enumerate() {
        println!("  Token {}: {:?}", i, token.kind);
    }

    // 测试更复杂的代码
    let complex_input = r#"
        function add(a, b)
            return a + b
        end
    "#;

    let mut session2 = oak_core::parser::ParseSession::<LuaLanguage>::default();
    lexer.lex(complex_input, &[], &mut session2);

    println!("✓ 复杂代码词法分析成功");
    let tokens_vec2 = session2.tokens.get_ref();
    println!("  生成的 token 数量: {}", tokens_vec2.len());
    assert!(!tokens_vec2.is_empty());

    println!("词法分析器测试完成");
}
