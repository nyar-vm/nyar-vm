use super::*;

const INPUT: &str = r#"
assign torch
assign numpy as np
assign pandas.database as db

assign .mod
assign ..mod as z
assign lib.*
assign mod::*
assign "./mod" as y
assign "../lib/mod"::{
	a as b
	c as d
	e.f.{g as h}
}
"#;

#[test]
fn debug_import() {
    unimplemented!()
    // get_ast(INPUT);
}
