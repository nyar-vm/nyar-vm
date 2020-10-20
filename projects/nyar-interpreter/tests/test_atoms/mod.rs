use super::*;
use nyar_interpreter::engine::NyarEngine;
use nyar_hir::ASTNode;

const NUMBERS: &str = r#"
0
// 0.
// .0
// 123456.
// .789
0.0
42f32
2147483647i32
9223372036854775807i64
170141183460469231731687303715884105727i128
57896044618658097711785492504343953926634992332820282019728792003956564819967
123456
123456i
1234.56im
1234.56cm
"#;

#[test]
fn debug_numbers() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(NUMBERS);
    let mut engine = NyarEngine::default();
    let out = engine.evaluate(&ASTNode::from(ast));
    println!("{:#?}", out);
    Ok(())
}

const SPECIALS: &str = r#"
null
true
false
"#;

#[test]
fn debug_specials() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(SPECIALS);
    ast.save("tests/test_atoms/debug_specials.yaml")
}
