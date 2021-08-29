use super::*;

mod test_apply;
mod test_dot;

const SIMPLE: &str = r#"
Call(a, b: b)
"#;

#[test]
fn debug_simple() {
    let _: ASTKind = ASTDump::parse(SIMPLE);
}
