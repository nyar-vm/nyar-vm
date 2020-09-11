use std::{collections::HashMap, rc::Weak};

use crate::value::function::NyarFunction;
use crate::value::variable::Variable;

pub struct ModuleInstance {
    father: Option<Weak<Self>>,
    variable_table: HashMap<String, Variable>,
}
