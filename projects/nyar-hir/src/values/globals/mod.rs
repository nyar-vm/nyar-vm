use crate::{Identifier, NyarValue};
use indexmap::IndexMap;

mod iters;
#[derive(Default)]
pub struct GlobalBuilder {
    items: IndexMap<String, NamedValue>,
}

pub struct NamedValue {
    constant: bool,
    name: Identifier,
    value: NyarValue,
}

impl GlobalBuilder {
    pub fn insert(&mut self, item: NamedValue) -> Option<NamedValue> {
        self.items.insert(item.name.to_string(), item)
    }
}

impl NamedValue {
    /// Create a new [`i32`] value
    pub fn i32(name: Identifier, value: i32) -> Self {
        Self { constant: false, name, value: NyarValue::I32(value) }
    }
    pub fn i64(name: Identifier, value: i32) -> Self {
        Self { constant: false, name, value: NyarValue::I32(value) }
    }
    pub fn f32(name: Identifier, value: f32) -> Self {
        Self { constant: false, name, value: NyarValue::F32(value) }
    }
    pub fn f64(name: Identifier, value: f32) -> Self {
        Self { constant: false, name, value: NyarValue::F32(value) }
    }
    pub fn function(name: Identifier) -> Self {
        Self { constant: false, name: name.clone(), value: NyarValue::Function(name) }
    }
    pub fn mutable(&self) -> bool {
        !self.constant
    }

    pub fn value(&self) -> &NyarValue {
        &self.value
    }
}
