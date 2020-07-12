use super::*;

#[derive(Debug, Clone)]
pub struct NyarFunction {
    /// f
    pub(crate) name: String,
    ///
    pub(crate) attributes: Option<NyarFunctionAttributes>,
    /// inline f(...)
    pub(crate) modifiers: Vec<String>,
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
