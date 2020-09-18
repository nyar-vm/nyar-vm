use crate::typing::Typing;
use std::{collections::HashMap, rc::Rc};
use std::task::Context;

pub trait Class {
    fn get_meta(&self) -> NyarClass;
}

impl Clone for Box<dyn Class> {
    fn clone(&self) -> Box<dyn Class> {
        self.clone()
    }
}

impl<T> PartialEq<T> for Box<dyn Class> {
    fn eq(&self, _other: &T) -> bool {
        todo!()
    }
}

impl Eq for Box<dyn Class> {}

pub struct NyarProperty {
    // typing: Option<Typing>,
    default: Rc<NyarClass>,
}

pub struct NyarClass {
    name: String,
    base: Vec<Rc<NyarClass>>,
    properties: HashMap<String, NyarProperty>,
    methods: HashMap<String, NyarProperty>,
}

impl Default for NyarClass {
    fn default() -> Self {
        Self {
            name: String::from("Object"),
            base: vec![],
            properties: Default::default(),
            methods: Default::default(),
        }
    }
}

impl NyarClass {
    pub fn inherit() {}
}

pub enum NyarIndexSystem {
    /// starts from 1
    OrdinalSystem,
    /// starts form 0
    OffsetSystem,
}

pub struct NyarContext {
    ///
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
    implicit_self: bool,
    /// let a = [1, 2]
    /// #index_system(0)
    /// print(a[1]) // 2
    /// #index_system(1)
    /// print(a[1]) // 1
    index_system: NyarIndexSystem,

    /// a.x => a.x()
    uniform_function_call_syntax: bool,
}

impl Default for NyarContext {
    fn default() -> Self {
        Self {
            implicit_self: false,
            index_system: NyarIndexSystem::OrdinalSystem,
            uniform_function_call_syntax: false.then_some()
        }
    }
}

pub struct NameSpace {
    name: String,
    base: Option<NameSpace>,
    ctx: NyarContext,
    classes: HashMap<String, Rc<NyarClass>>,
}
