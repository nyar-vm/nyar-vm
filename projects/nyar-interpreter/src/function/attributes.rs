use super::*;
use std::lazy::SyncLazy;

#[derive(Debug, Clone)]
pub struct NyarFunctionAttributes {
    is_currying: bool,
}

impl Default for NyarFunctionAttributes {
    fn default() -> Self {
        Self { is_currying: true }
    }
}

pub static NYAR_FUNCTION_ATTRIBUTES: SyncLazy<NyarFunctionAttributes> = SyncLazy::new(|| NyarFunctionAttributes::default());

impl NyarFunction {
    pub fn attributes(&self) -> &NyarFunctionAttributes {
        match &self.attributes {
            None => &NYAR_FUNCTION_ATTRIBUTES,
            Some(s) => s,
        }
    }
}

impl FunctionInstance {
    #[inline]
    pub fn is_currying(&self) -> bool {
        self.prototype.attributes().is_currying
    }
}
