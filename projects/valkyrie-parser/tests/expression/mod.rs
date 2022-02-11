use crate::debug_lexer;
use valkyrie_errors::ValkyrieResult;

#[test]
fn test_unary() {
    // debug_lexer(&[
    //     "tests/literal/symbol.vk", "tests/literal/number.vk", "tests/literal/string.vk", "tests/literal/escape.vk"])
    //     .unwrap();
}

#[test]
fn test_binary() -> ValkyrieResult {
    debug_lexer(&[
        "tests/expression/infix1.vk",
        "tests/expression/infix2.vk",
        "tests/expression/infix3.vk",
        "tests/expression/infix4.vk",
    ])
}
