use crate::modifiers::NyarReadWrite;

pub trait NyarClass {
    fn is_native(&self) -> bool {
        true
    }

    fn get_name(&self) -> String;

    fn get_access(&self) -> NyarReadWrite;
}

impl NyarClass for u8 {
    fn get_name(&self) -> String {
        "Unsigned8".to_string()
    }

    fn get_access(&self) -> NyarReadWrite {
        NyarReadWrite::Public
    }
}

pub struct CustomClass {
    name: String,
    access: NyarReadWrite,
}

impl NyarClass for CustomClass {
    fn get_name(&self) -> String {
        todo!()
    }

    fn get_access(&self) -> NyarReadWrite {
        todo!()
    }
}
