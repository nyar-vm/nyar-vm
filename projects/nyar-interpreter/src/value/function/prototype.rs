use super::*;
use crate::typing::Typing;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NyarFunction {
    /// f
    pub(crate) name: String,
    ///
    pub(crate) attributes: Option<Box<NyarFunctionAttributes>>,
    /// inline f(...)
    /// pub(crate) modifiers: Vec<String>,
    /// f(self,...)
    pub(crate) with_self: Argument,
    /// f<T>(...)
    pub(crate) generic: Vec<Statement>,
    /// f(a, b, c)
    pub(crate) arguments: IndexMap<String, Argument>,
    /// f(a, b, c, < , ...)
    pub(crate) position_only: Option<IndexMap<String, Argument>>,
    /// f(..., >, a, b, c)
    pub(crate) keywords_only: Option<IndexMap<String, Argument>>,
    /// f(..list: T)
    pub(crate) collect_list: Option<(String, Typing)>,
    /// f(...dict: T)
    pub(crate) collect_dict: Option<(String, Typing)>,
    /// f(...): T
    pub(crate) return_type: Typing,
    /// f(...): / {E}
    pub(crate) effects: IndexMap<String, Rc<EffectHandler>>,
    /// f<T, E>(...): T / {E} where ...
    pub(crate) where_bounds: Vec<Statement>,
    /// f(...) {}
    pub(crate) body: Statement,
}

pub enum FunctionStatement {
    Nyar,
    Native,
}

impl NyarFunction {
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
