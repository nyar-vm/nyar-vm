use std::collections::{BTreeMap, LinkedList};

use nyar_error::Span;
use nyar_hir::ast::Symbol;

use crate::{typing::Typing, value::Symbol, SymbolColor};

use super::*;

mod builder;

pub struct FunctionDispatcher {
    name: String,
    prototypes: LinkedList<FunctionPrototype>,
}

pub struct LambdaFunction {
    prototype: FunctionPrototype,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionPrototype {
    /// Jump to which position
    pub definition_span: Span,
    /// Show which color
    pub color: SymbolColor,
    ///
    pub attributes: FunctionAttributes,
    /// ```vk
    /// inline f(...)
    /// public modifiers: Vector[String],
    /// f(self,...)
    /// ```
    pub with_self: FunctionParameter,
    /// ```vk
    /// f[T](...)
    /// ```
    pub generic: Vec<Statement>,
    /// ```vk
    /// f(a, b, c)
    /// ```
    pub arguments: IndexMap<String, FunctionParameter>,
    /// ```vk
    /// f(a, b, c, < , ...)
    /// ```
    pub position_only: IndexMap<String, FunctionParameter>,
    /// ```vk
    /// f(..., >, a, b, c)
    /// ```
    pub keywords_only: IndexMap<String, FunctionParameter>,
    /// ```vk
    /// f(..list: T)
    /// ```
    pub collect_list: Option<(String, Typing)>,
    /// ```vk
    /// f(...dict: T)
    /// ```
    pub collect_dict: Option<(String, Typing)>,
    /// ```vk
    /// f(...): T
    /// ```
    pub return_type: Typing,
    /// ```vk
    /// f(...): / {E}
    /// ```
    pub effects: IndexMap<String, Rc<EffectHandler>>,
    /// ```vk
    /// f[T, E](...): T / [E] where ...
    /// ```
    pub where_bounds: Vec<Statement>,
    /// ```vk
    /// f(...) {}
    /// ```
    pub body: Statement,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionParameter {}

pub enum FunctionStatement {
    Nyar,
    Native,
}

impl FunctionPrototype {
    fn check_attributes(&mut self) {
        if self.attributes.is_none() {
            self.attributes = Default::default()
        }
    }

    pub fn set_currying(&mut self, level: i8) {
        self.check_attributes();
        let v = &mut self.attributes.as_mut().unwrap().currying;
        match level {
            0 => (),
            x if x > 0 => *v = true,
            _ => *v = false,
        }
    }
    pub fn set_override_keywords(&mut self, level: i8) {
        self.check_attributes();
        let v = &mut self.attributes.as_mut().unwrap().override_keywords;
        match level {
            0 => (),
            x if x > 0 => *v = ErrorLevels::Allow,
            -1 => *v = ErrorLevels::Warning,
            _ => *v = ErrorLevels::Deny,
        }
    }
}
