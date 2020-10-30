use super::*;
use crate::Value;


mod integer_handler;
mod decimal_handler;

pub use self::{
    integer_handler::{DefaultIntegerHandler,BUILD_INTEGER_PARSERS},
    decimal_handler::{DefaultDecimalHandler,BUILD_DECIMAL_PARSERS},
};

#[derive(Copy, Clone, Debug)]
pub enum NyarIndexSystem {
    /// starts from 1
    OrdinalSystem,
    /// starts form 0
    OffsetSystem,
}

pub type StringCallback = fn(String) -> Result<Value>;

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

    pub default_integer_handler: Option<DefaultIntegerHandler>,

    pub default_decimal_handler: Option<DefaultDecimalHandler>,

    pub default_string_handler: Option<bool>,

    pub missing_access_modifier_handler: Option<AccessModifierHandler>,
}

#[derive(Clone, Debug)]
pub enum AccessModifierHandler {
    Public,
    Private,
    Custom(),
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
            default_decimal_handler: None,
            default_string_handler: None,
            missing_access_modifier_handler: None,
        }
    }
}

pub static NYAR_CONTEXT_PRESET: SyncLazy<NyarContext> = SyncLazy::new(|| NyarContext {
    implicit_self: Some(false),
    index_system: Some(NyarIndexSystem::OrdinalSystem),
    uniform_function_call_syntax: Some(true),
    clean_inherit_modules: Some(false),
    clean_prelude_modules: Some(false),
    default_integer_handler: Some(*BUILD_INTEGER_PARSERS),
    default_decimal_handler: Some(*BUILD_DECIMAL_PARSERS),
    default_string_handler: None,
    missing_access_modifier_handler: None,
});

macro_rules! wrap_context {
    ($p:ident,$f_get:ident,$f_set:ident, $t:ty) => {
        pub fn $f_get(&self) -> $t {
            match self.context.$p {
                Some(s) => s,
                None => match &self.father {
                    Some(s) => s.upgrade().unwrap().$f_get(),
                    None => NYAR_CONTEXT_PRESET.$p.unwrap(),
                },
            }
        }
        pub fn $f_set(&mut self, new: $t) {
            self.context.$p = Some(new)
        }
    }; /* ($p:ident, $t:ty) => {
        *     wrap_context!($p, concat_idents!(get_,$p), concat_idents!(set_,$p), $t);
        * }; */
}

impl ModuleInstance {
    // pub fn get_implicit_self(&self) -> bool {
    //     match self.context.implicit_self {
    //         Some(s) => s,
    //         None => match &self.father {
    //             Some(s) => s.upgrade().unwrap().ctx_implicit_self(),
    //             None => NYAR_CONTEXT_PRESET.implicit_self.unwrap(),
    //         },
    //     }
    // }
    // pub fn set_implicit_self(&mut self, new: bool) {
    //     self.context.implicit_self = Some(new)
    // }
    wrap_context!(implicit_self, get_implicit_self, set_implicit_self, bool);
    wrap_context!(uniform_function_call_syntax, get_ufcs, set_ufcs, bool);
    wrap_context!(index_system, get_index_system, set_index_system, NyarIndexSystem);
    wrap_context!(default_integer_handler, get_integer_handler, set_integer_handler, DefaultIntegerHandler);
    wrap_context!(default_decimal_handler, get_decimal_handler, set_decimal_handler, DefaultDecimalHandler);
}
