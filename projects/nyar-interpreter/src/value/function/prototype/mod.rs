use std::collections::{BTreeMap, LinkedList};

use nyar_error::Span;
use nyar_hir::ast::Symbol;

use crate::{typing::Typing, value::Symbol};

use super::*;

mod builder;

pub struct FunctionDispatcher {
    name: Option<Symbol>,
    prototypes: LinkedList<FunctionPrototype>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionPrototype {
    name: Option<String>,
    /// Jump to which position
    definition_span: Span,
    ///
    pub attributes: Option<Box<FunctionAttributes>>,
    /// ```vk
    /// inline f(...)
    /// pub modifiers: Vector[String],
    /// f(self,...)
    /// ```
    pub with_self: Argument,
    /// f<T>(...)
    pub generic: Vec<Statement>,
    /// f(a, b, c)
    pub arguments: IndexMap<String, Argument>,
    /// f(a, b, c, < , ...)
    pub position_only: Option<IndexMap<String, Argument>>,
    /// f(..., >, a, b, c)
    pub keywords_only: Option<IndexMap<String, Argument>>,
    /// f(..list: T)
    pub collect_list: Option<(String, Typing)>,
    /// f(...dict: T)
    pub collect_dict: Option<(String, Typing)>,
    /// f(...): T
    pub return_type: Typing,
    /// f(...): / {E}
    pub effects: IndexMap<String, Rc<EffectHandler>>,
    /// ```vk
    /// f[T, E](...): T / [E] where ...
    /// ```
    pub where_bounds: Vec<Statement>,
    /// f(...) {}
    pub body: Statement,
}

pub enum FunctionStatement {
    Nyar,
    Native,
}

impl Default for FunctionPrototype {
    fn default() -> Self {
        todo!()
    }
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
