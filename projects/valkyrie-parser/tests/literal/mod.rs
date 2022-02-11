use crate::debug_lexer;

#[test]
fn test_atomic() {
    debug_lexer(&["tests/literal/symbol.vk", "tests/literal/number.vk", "tests/literal/string.vk", "tests/literal/escape.vk"])
        .unwrap();
}

#[test]
fn test_composite() {
    debug_lexer(&["tests/literal/tuple.vk", "tests/literal/table.vk"]).unwrap();
}
