use crate::{NyarClass, NyarValue};

impl NyarValue {
    pub fn new_class(object: impl NyarClass + 'static) -> Self {
        todo!()
    }
}

pub trait NyarCast {
    fn to_value(self) -> NyarValue;
    fn as_boolean(&self) -> Option<bool> {
        None
    }
    fn as_u8(&self) -> Option<u8> {
        None
    }
    fn as_u16(&self) -> Option<u16> {
        None
    }

    fn as_i64(&self) -> Option<u16> {
        None
    }
}
