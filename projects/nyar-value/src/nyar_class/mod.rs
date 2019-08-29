use crate::{modifiers::NyarReadWrite, NyarCast, NyarValue};

mod customs;

pub trait NyarClass: NyarCast {
    fn is_native(&self) -> bool {
        true
    }
    fn get_name(&self) -> &str;
    fn get_access(&self) -> NyarReadWrite;
}

impl NyarCast for u8 {
    fn to_value(self) -> NyarValue {
        NyarValue::new_class(self)
    }
    fn as_u8(&self) -> Option<u8> {
        Some(*self)
    }
}

impl NyarClass for u8 {
    fn get_name(&self) -> &str {
        "Unsigned8"
    }

    fn get_access(&self) -> NyarReadWrite {
        NyarReadWrite::Public
    }
}

pub struct CustomClass {
    name: String,
    access: NyarReadWrite,
}

impl NyarCast for CustomClass {
    fn to_value(self) -> NyarValue {
        todo!()
    }
}

impl NyarClass for CustomClass {
    fn is_native(&self) -> bool {
        false
    }

    fn get_name(&self) -> &str {
        self.name.as_str()
    }

    fn get_access(&self) -> NyarReadWrite {
        todo!()
    }
}
