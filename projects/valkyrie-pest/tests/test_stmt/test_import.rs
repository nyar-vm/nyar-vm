use super::*;

const INPUT: &str = r#"
using std;
using `python`.numpy as npy
using {
    \\ wow!
    `python`.torch as tf
}
using "script/path" {
    self as script, other
}
using lib1.*
using lib2::*
using lib3 as z
using lib4 as _
using lib5.{
	a as b
	c as d
	e::f::{
        g as h
	}
}
"#;

#[test]
fn debug_import() -> Result<()> {
    let ast: ASTKind = ASTDump::parse(INPUT);
    ast.save("tests/test_stmt/debug_import.clj")
}
