use super::*;

#[test]
fn test_unary() {
    // debug_lexer(&[
    //     "tests/literal/symbol.vk", "tests/literal/number.vk", "tests/literal/string.vk", "tests/literal/escape.vk"])
    //     .unwrap();
}

#[test]
fn test_class() -> ValkyrieResult {
    debug_lexer(&["tests/declaration/class1.vk"])
}
