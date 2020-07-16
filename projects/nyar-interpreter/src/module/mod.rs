use crate::{function::NyarFunction, variable::Variable};
use std::{collections::HashMap, rc::Weak};

pub struct ModuleInstance {
    father: Option<Weak<Self>>,
    variable_table: HashMap<String, Variable>,
}
