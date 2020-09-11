use super::*;
use crate::value::error::Level3;
use std::lazy::SyncLazy;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NyarFunctionAttributes {
    pub currying: bool,
    pub extra_arguments: Level3,
    pub extra_keywords: Level3,
    pub override_keywords: Level3,
}

impl Default for NyarFunctionAttributes {
    fn default() -> Self {
        Self {
            currying: true,
            extra_arguments: Level3::Warning,
            extra_keywords: Level3::Warning,
            override_keywords: Level3::Warning,
        }
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
        self.prototype.attributes().currying
    }
    #[inline]
    pub fn allow_extra_arguments(&self) -> Level3 {
        self.prototype.attributes().extra_arguments
    }
    #[inline]
    pub fn allow_extra_keywords(&self) -> Level3 {
        self.prototype.attributes().extra_keywords
    }
    #[inline]
    pub fn allow_override_keywords(&self) -> Level3 {
        self.prototype.attributes().override_keywords
    }
}
