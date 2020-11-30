use super::*;
use crate::Value;

mod decimal_handler;
mod integer_handler;

pub use self::{
    decimal_handler::{parse_f32, parse_f64, DefaultDecimalHandler, BUILD_IN_DECIMAL_PARSERS},
    integer_handler::{DefaultIntegerHandler, BUILD_IN_INTEGER_PARSERS},
};
use crate::{engine::NyarEngine, value::class::NyarReadWrite};
use std::lazy::SyncLazy;

#[derive(Copy, Clone, Debug)]
pub enum NyarIndexSystem {
    /// starts from 1
    OrdinalSystem,
    /// starts form 0
    OffsetSystem,
}

pub type StringCallback = fn(&str) -> Result<Value>;

#[derive(Clone, Debug)]
pub struct NyarContext {
    /// ```vk
    /// class Example {
    ///     a: int = 0
    /// }
    ///
    /// extends Example {
    ///     f() {
    ///         print(self.a)
    ///         print(a)
    ///     }
    /// }
    /// ```
    pub implicit_self: Option<bool>,
    /// ```vk
    /// let a = [1, 2]
    /// #index_system(0)
    /// print(a[1]) // 2
    /// #index_system(1)
    /// print(a[1]) // 1
    /// ```
    pub index_system: Option<NyarIndexSystem>,
    /// a.x => a.x()
    pub uniform_function_call_syntax: Option<bool>,

    pub clean_inherit_modules: Option<bool>,

    pub clean_prelude_modules: Option<bool>,

    pub default_integer_handler: Option<String>,

    pub integer_handlers: Option<DefaultIntegerHandler>,

    pub default_decimal_handler: Option<String>,

    pub decimal_handlers: Option<DefaultDecimalHandler>,

    pub default_string_handler: Option<bool>,

    pub default_access_handler: Option<AccessModifierHandler>,
}

#[derive(Clone, Debug)]
pub enum AccessModifierHandler {
    Public,
    Private,
    Custom(NyarReadWrite),
}

impl Default for NyarContext {
    fn default() -> Self {
        Self {
            implicit_self: None,
            index_system: None,
            uniform_function_call_syntax: None,
            clean_inherit_modules: None,
            clean_prelude_modules: None,
            default_integer_handler: None,
            integer_handlers: None,
            default_decimal_handler: None,
            decimal_handlers: None,
            default_string_handler: None,
            default_access_handler: None,
        }
    }
}

pub static NYAR_CONTEXT_PRESET: SyncLazy<NyarContext> = SyncLazy::new(|| NyarContext {
    implicit_self: Some(false),
    index_system: Some(NyarIndexSystem::OrdinalSystem),
    uniform_function_call_syntax: Some(true),
    clean_inherit_modules: Some(false),
    clean_prelude_modules: Some(false),
    default_integer_handler: Some(String::from("int")),
    integer_handlers: Some((*BUILD_IN_INTEGER_PARSERS).clone()),
    default_decimal_handler: Some(String::from("dec")),
    decimal_handlers: Some((*BUILD_IN_DECIMAL_PARSERS).clone()),
    default_string_handler: None,
    default_access_handler: None,
});

macro_rules! wrap_context {
    ($p:ident,$f_get:ident,$f_set:ident, $t:ty) => {
        impl ModuleInstance {
            pub fn $f_get(&self, pkg: &ModuleManager) -> $t {
                if let Some(v) = &self.context.$p {
                    return v.to_owned();
                }
                for m in pkg.get_ancestors_modules().iter().rev() {
                    if let Some(v) = &m.context.$p {
                        return v.to_owned();
                    }
                }
                return NYAR_CONTEXT_PRESET.$p.to_owned().unwrap();
            }
            pub fn $f_set(&mut self, new: $t) {
                self.context.$p = Some(new)
            }
        }

        impl NyarEngine {
            #[inline]
            pub fn $f_get(&self) -> $t {
                self.current_pkg.get_current_module().$f_get(&self.current_pkg)
            }
            #[inline]
            pub fn $f_set(&mut self, new: $t) {
                self.current_pkg.get_current_module_mut().$f_set(new)
            }
        }
    };
}

impl ModuleInstance {
    pub fn get_implicit_self(&self, pkg: &ModuleManager) -> bool {
        if let Some(v) = &self.context.implicit_self {
            return v.to_owned();
        }
        for m in pkg.get_ancestors_modules().iter().rev() {
            if let Some(v) = &m.context.implicit_self {
                return v.to_owned();
            }
        }
        return NYAR_CONTEXT_PRESET.implicit_self.to_owned().unwrap();
    }
    pub fn set_implicit_self(&mut self, new: bool) {
        self.context.implicit_self = Some(new)
    }
}

impl NyarEngine {
    #[inline]
    pub fn get_implicit_self(&self) -> bool {
        self.current_pkg.get_current_module().get_implicit_self(&self.current_pkg)
    }
    #[inline]
    pub fn set_implicit_self(&mut self, new: bool) {
        self.current_pkg.get_current_module_mut().set_implicit_self(new)
    }
}

// wrap_context!(implicit_self, get_implicit_self, set_implicit_self, bool);
wrap_context!(uniform_function_call_syntax, get_ufcs, set_ufcs, bool);
wrap_context!(index_system, get_index_system, set_index_system, NyarIndexSystem);
wrap_context!(default_integer_handler, get_integer_handler, set_integer_handler, String);
wrap_context!(default_decimal_handler, get_decimal_handler, set_decimal_handler, String);
wrap_context!(integer_handlers, get_integer_handlers, set_integer_handlers, DefaultIntegerHandler);
wrap_context!(decimal_handlers, get_decimal_handlers, set_decimal_handlers, DefaultDecimalHandler);
