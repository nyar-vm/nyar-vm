use valkyrie_errors::ValkyrieResult;

use crate::debug_lexer;

#[test]
fn test_atomic() -> ValkyrieResult {
    debug_lexer(&["tests/literal/symbol.vk", "tests/literal/binary.vk"])
}

#[test]
fn test_number() -> ValkyrieResult {
    debug_lexer(&["tests/literal/number.vk"])
}

#[test]
fn test_string() -> ValkyrieResult {
    debug_lexer(&["tests/literal/string.vk", "tests/literal/escape.vk"])
}

#[test]
fn test_composite() {
    debug_lexer(&["tests/literal/tuple.vk", "tests/literal/table.vk"]).unwrap();
}
