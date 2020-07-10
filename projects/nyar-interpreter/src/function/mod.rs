
use std::rc::Rc;

struct Continuation<'a, V>(&'a dyn Fn(&Continuation<V>));

struct EffectHandler {
    effects: Hashmap<String, >
}

struct Argument;
struct Statement;
struct Typing;
enum Value {

}

#[derive(Default,Clone)]
pub struct NyarFunctionAttributes {
    is_currying: bool
}

pub struct NyarFunction {
    /// f
    name: String,
    ///
    attributes : Option<NyarFunctionAttributes>,
    /// inline f(...)
    modifiers: Vec<String>,
    /// f(self,...)
    with_self: Argument,
    /// f<T>(...)
    generic: Vec<Statement>,
    /// f(a, b, c)
    arguments: IndexMap<String, Argument>,
    /// f(a, b, c, < , ...)
    position_only: Option<IndexMap<String, Argument>>,
    /// f(..., >, a, b, c)
    keywords_only: Option<IndexMap<String, Argument>>,
    /// f(..list: T)
    collect_list: Option<(String, Typing)>,
    /// f(...dict: T)
    collect_dict: Option<(String, Typing)>,
    /// f(...): T
    return_type: Typing,
    /// f(...): / {E}
    effects: IndexMap<String, Rc<EffectHandler>>,
    /// f<T, E>(...): T / {E} where ...
    where_bounds: Vec<Statement>,
    /// f(...) {}
    body: Statement
}
impl NyarFunction {
    pub fn attributes(&self) -> &NyarFunctionAttributes {
        match &self.attributes {
            None => { &Default::default() }
            Some(s) => {s}
        }
    }
}

struct FunctionInstance {
    prototype: Rc<NyarFunction>,
    args: Vec<Value>,
    kvs: IndexMap<String, Value>,
}

impl FunctionInstance {
    pub fn new(f: NyarFunction) {
        Self {

        }
    }
    pub fn fill_arguments(&mut self) {

    }
    pub fn fill_named_arguments(&mut self) {

    }
}