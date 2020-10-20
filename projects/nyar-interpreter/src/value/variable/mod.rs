use self::attributes::VariableAttributes;
use crate::Value;

mod attributes;

#[derive(Clone, Debug)]
pub struct Variable {
    instance: Value,
    attributes: Option<Box<VariableAttributes>>,
}

impl Variable {
    pub fn is_mutable(&self)->bool {
        true
    }
}