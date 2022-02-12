use valkyrie_errors::ValkyrieResult;

use crate::debug_lexer;

#[test]
fn test_atomic() -> ValkyrieResult {
    debug_lexer(&[
        "tests/literal/symbol.vk",
        "tests/literal/number.vk",
        "tests/literal/binary.vk",
        "tests/literal/string.vk",
        "tests/literal/escape.vk",
    ])
}

#[test]
fn test_composite() {
    debug_lexer(&["tests/literal/tuple.vk", "tests/literal/table.vk"]).unwrap();
}
