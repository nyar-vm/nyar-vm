use super::*;

const INPUT: &str = r#"
using torch
using numpy as np
using pandas.database as pd

using mod
using mod as z
using lib.*
using mod::*
using mod as y
using mod::{
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
