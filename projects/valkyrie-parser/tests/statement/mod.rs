use valkyrie_errors::ValkyrieResult;

use crate::debug_lexer;

#[test]
fn test_for() -> ValkyrieResult {
    debug_lexer(&["tests/statement/for_loop.vk"])
}

#[test]
fn test_while() -> ValkyrieResult {
    debug_lexer(&["tests/statement/while.vk"])
}

#[test]
fn test_string() -> ValkyrieResult {
    debug_lexer(&[])
}

#[test]
fn test_composite() -> ValkyrieResult {
    debug_lexer(&[])
}
