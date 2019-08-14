mod customs;

use std::collections::HashMap;
use crate::modifiers::NyarReadWrite;

pub trait NyarClass {
    fn is_native(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str;

    fn get_access(&self) -> NyarReadWrite;
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
    methods: HashMap<String, String>
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
