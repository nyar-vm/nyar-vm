use super::*;
use crate::value::error::ErrorLevels;
use nyar_error::ErrorLevels;
use std::lazy::SyncLazy;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionAttributes {
    pub currying: bool,
    pub extra_arguments: ErrorLevels,
    pub extra_keywords: ErrorLevels,
    pub override_keywords: ErrorLevels,
}

impl Default for FunctionAttributes {
    fn default() -> Self {
        Self {
            currying: true,
            extra_arguments: ErrorLevels::Warning,
            extra_keywords: ErrorLevels::Warning,
            override_keywords: ErrorLevels::Warning,
        }
    }
}

pub static NYAR_FUNCTION_ATTRIBUTES: SyncLazy<FunctionAttributes> = SyncLazy::new(|| FunctionAttributes::default());

impl FunctionPrototype {
    pub fn attributes(&self) -> &FunctionAttributes {
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
    pub fn allow_extra_arguments(&self) -> ErrorLevels {
        self.prototype.attributes().extra_arguments
    }
    #[inline]
    pub fn allow_extra_keywords(&self) -> ErrorLevels {
        self.prototype.attributes().extra_keywords
    }
    #[inline]
    pub fn allow_override_keywords(&self) -> ErrorLevels {
        self.prototype.attributes().override_keywords
    }
}
