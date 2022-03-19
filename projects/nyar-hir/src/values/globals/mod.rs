use crate::{Identifier, NyarValue};
use indexmap::IndexMap;

mod iters;
#[derive(Default)]
pub struct GlobalBuilder {
    items: IndexMap<String, NamedValue>,
}

pub struct NamedValue {
    pub namepath: Identifier,
    pub constant: bool,
    pub export: bool,
    pub value: NyarValue,
}

impl GlobalBuilder {
    pub fn insert(&mut self, item: NamedValue) -> Option<NamedValue> {
        self.items.insert(item.namepath.to_string(), item)
    }
}

impl NamedValue {
    /// Create a new [`i32`] value
    pub fn i32(name: Identifier, value: i32) -> Self {
        Self { namepath: name, value: NyarValue::I32(value), constant: false, export: false }
    }
    pub fn i64(name: Identifier, value: i32) -> Self {
        Self { namepath: name, value: NyarValue::I32(value), constant: false, export: false }
    }
    pub fn f32(name: Identifier, value: f32) -> Self {
        Self { namepath: name, value: NyarValue::F32(value), constant: false, export: false }
    }
    pub fn f64(name: Identifier, value: f32) -> Self {
        Self { namepath: name, value: NyarValue::F32(value), constant: false, export: false }
    }
    pub fn function(name: Identifier) -> Self {
        Self { namepath: name.clone(), value: NyarValue::Function(name), constant: false, export: false }
    }
    pub fn mutable(&self) -> bool {
        !self.constant
    }

    pub fn with_public(self) -> Self {
        Self { export: true, ..self }
    }

    pub fn value(&self) -> &NyarValue {
        &self.value
    }
}
