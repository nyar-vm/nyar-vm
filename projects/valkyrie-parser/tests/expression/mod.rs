use std::str::FromStr;
use valkyrie_errors::ValkyrieResult;
use valkyrie_parser::{testing::debug_lexer, ValkyrieOperator};

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

#[test]
fn operators() {
    ValkyrieOperator::from_str("+").unwrap();
}
