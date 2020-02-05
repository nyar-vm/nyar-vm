extern crate nyar_valkyrie;
use nyar_valkyrie::{get_ast, get_statements};

const INPUT: &str = r#"
@py import numpy as np
@py import pandas.dataframe

import .mod as x
import ..mod as z
import lib.*
import mod::*
import "./mod" as y
import "../lib/mod"::{
	a as b
	c as d
	e.f.{g as h}
}
"#;

#[test]
fn debug_import() {
    get_ast(INPUT);
    assert_eq!(0, 1)
}
