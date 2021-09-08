use super::*;

#[test]
fn test_basic() {
    run_parser(&["tests/literal/symbol.vk", "tests/literal/number.vk", "tests/literal/string.vk", "tests/literal/escape.vk"])
        .unwrap();

    run_parser(&["tests/literal/tuple.vk", "tests/literal/table.vk"]).unwrap();
}
