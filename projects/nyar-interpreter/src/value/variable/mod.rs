use self::attributes::VariableAttributes;
use crate::Value;

mod attributes;

pub struct Variable {
    instance: Value,
    attributes: Option<Box<VariableAttributes>>,
}
