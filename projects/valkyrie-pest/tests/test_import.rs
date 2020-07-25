extern crate nyar_valkyrie;
use nyar_valkyrie::get_ast;

const INPUT: &str = r#"
import torch
import numpy as np
import pandas.database as db

import .mod
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
}
