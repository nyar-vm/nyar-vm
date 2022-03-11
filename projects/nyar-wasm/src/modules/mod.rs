use nyar_error::FileSpan;
use std::rc::Rc;

pub struct GlobalValue {
    pub(crate) name: Identifier,
    pub(crate) typing: NyarType,
    pub(crate) value: NyarValue,
}

pub enum NyarType {
    I32,
    F32,
}

pub enum NyarValue {
    I32(i32),
    F32(f32),
}

impl GlobalValue {
    pub fn new_i32(name: Identifier, value: i32) -> Self {
        Self { name, typing: NyarType::I32, value: NyarValue::I32(value) }
    }
    pub fn new_f32<S: ToString>(name: S, value: f32) -> Self {
        Self { name: name.to_string(), typing: NyarType::F32, value: NyarValue::F32(value) }
    }
}
