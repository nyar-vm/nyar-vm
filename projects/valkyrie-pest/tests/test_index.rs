extern crate nyar_valkyrie;

use nyar_valkyrie::get_ast;

const INPUT: &str = r#"
a[1]

a   [
    2
    ]

a
[3]
"#;

#[test]
fn debug_index() -> std::io::Result<()> {
    let ast = get_ast(INPUT);
    ast.save("tests/debug_slice.yaml")
}
