//! C language smoke test.

use std::collections::HashMap;

use legend_vm::LegendVmRunner;

#[test]
fn c_printf_and_add() {
    let runner = LegendVmRunner::new();
    let mut env = HashMap::new();

    let source = r#"
#include <stdio.h>
int add(int a, int b) {
    return a + b;
}
int main(void) {
    int x = add(40, 2);
    printf("%d\n", x);
    return 0;
}
"#;
    let result = runner.run("c", source, &mut env).expect("run");
    assert_eq!(result.to_string_value(), "42");
}

#[test]
fn detects_c_from_extension() {
    let language = LegendVmRunner::detect_language_from_file("demo.c", "int main(void) { return 0; }");
    assert_eq!(language.as_deref(), Some("c"));
}
