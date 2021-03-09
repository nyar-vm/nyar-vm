use super::*;
use crate::typing::Typing;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionPrototype {
    /// f
    pub name: String,
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
            x if x > 0 => *v = Level3::Allow,
            -1 => *v = Level3::Warning,
            _ => *v = Level3::Deny,
        }
    }
}
