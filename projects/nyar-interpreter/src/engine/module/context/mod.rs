use super::*;
use crate::Value;

mod decimal_handler;
mod integer_handler;

pub use self::{
    decimal_handler::{DefaultDecimalHandler, BUILD_IN_DECIMAL_PARSERS},
    integer_handler::{IntegerHandlerManager, BUILD_IN_INTEGER_PARSERS},
};
use crate::engine::{module::NyarReadWrite, NyarEngine};
use std::{lazy::SyncLazy, rc::Weak, sync::Arc};

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

    pub integer_handler: IntegerHandlerManager,

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
            integer_handler: Default::default(),
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
    integer_handler: Some(String::from("int")),
    integer_handler: Some((*BUILD_IN_INTEGER_PARSERS).clone()),
    default_decimal_handler: Some(String::from("dec")),
    decimal_handlers: Some((*BUILD_IN_DECIMAL_PARSERS).clone()),
    default_string_handler: None,
    default_access_handler: None,
});

impl ModuleInstance {
    pub fn get_context_mut(&mut self) -> &mut NyarContext {
        match &self.context {
            None => {
                let ctx = NyarContext::default();
                self.context = Some(box ctx);
            }
            Some(_) => {}
        }
        self.context.as_mut().unwrap().as_mut()
    }
}

macro_rules! wrap_context {
    ($p:ident,$f_get:ident,$f_set:ident, $t:ty) => {
        impl ModuleInstance {
            pub fn $f_get(&self, pkg: &ModuleManager) -> $t {
                match self.context.as_ref().and_then(|ctx| ctx.$p.as_ref()) {
                    Some(s) => return s.to_owned(),
                    None => {}
                }
                for shared in pkg.get_ancestors_modules().iter().rev() {
                    match shared.read().ok().as_ref().and_then(|ctx| ctx.context.as_ref()).and_then(|ctx| ctx.$p.as_ref()) {
                        Some(s) => return s.to_owned(),
                        None => {}
                    }
                }
                return NYAR_CONTEXT_PRESET.$p.to_owned().unwrap();
            }
            #[inline]
            pub fn $f_set(&mut self, new: $t) {
                self.get_context_mut().$p = Some(new)
            }
        }

        impl NyarEngine {
            #[inline]
            pub fn $f_get(&self) -> $t {
                match self.current_pkg.get_current_module().read() {
                    Ok(o) => o.$f_get(&self.current_pkg),
                    Err(_) => {
                        panic!()
                    }
                }
            }
            #[inline]
            pub fn $f_set(&mut self, new: $t) {
                match self.current_pkg.get_current_module().write() {
                    Ok(mut o) => o.$f_set(new),
                    Err(_) => {
                        panic!()
                    }
                }
            }
        }
    };
}

impl ModuleInstance {
    pub fn get_integer_handler(&self, pkg: &PackageManager) -> String {
        match self.context.as_ref().and_then(|ctx| ctx.integer_handler.as_ref()) {
            Some(s) => return s.to_owned(),
            None => {}
        }
        for shared in pkg.get_ancestors_modules().iter().rev() {
            match shared.read().ok().as_ref().and_then(|ctx| ctx.context.as_ref()).and_then(|ctx| ctx.integer_handler.as_ref())
            {
                Some(s) => return s.to_owned(),
                None => {}
            }
        }
        return NYAR_CONTEXT_PRESET.integer_handler.to_owned().unwrap();
    }
    #[inline]
    pub fn set_integer_handler(&mut self, new: String) {
        self.get_context_mut().integer_handler = Some(new)
    }
}

impl NyarEngine {
    #[inline]
    pub fn get_integer_handler(&self) -> String {
        match self.current_pkg.get_current_module().read() {
            Ok(mut o) => o.get_integer_handler(&self.current_pkg),
            Err(_) => {
                panic!()
            }
        }
    }
    #[inline]
    pub fn set_integer_handler(&mut self, new: String) {
        match self.current_pkg.get_current_module().write() {
            Ok(mut o) => o.set_integer_handler(new),
            Err(_) => {
                panic!()
            }
        }
    }
}

wrap_context!(implicit_self, get_implicit_self, set_implicit_self, bool);
wrap_context!(uniform_function_call_syntax, get_ufcs, set_ufcs, bool);
wrap_context!(index_system, get_index_system, set_index_system, NyarIndexSystem);
// wrap_context!(default_integer_handler, get_integer_handler, set_integer_handler, String);
wrap_context!(default_decimal_handler, get_decimal_handler, set_decimal_handler, String);
wrap_context!(integer_handler, get_integer_handlers, set_integer_handlers, IntegerHandlerManager);
wrap_context!(decimal_handlers, get_decimal_handlers, set_decimal_handlers, DefaultDecimalHandler);
