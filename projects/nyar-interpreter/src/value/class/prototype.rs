use super::*;
use std::sync::Arc;

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

pub struct NyarVisibility {
    self_read: bool,
    self_write: bool,
}

#[derive(Debug, Clone)]
pub struct NyarProperty {
    // typing: Option<Typing>,
    default: Arc<NyarClass>,
    visibility: NyarReadWrite,
}

pub struct MatchTree {
    base: BTreeMap<usize, CaseObject>,
    default: bool,
}

pub struct CaseObject {
    cond: bool,
    expr: bool,
}

pub enum NyarPrototype {
    EmptyClass,
    EmptyVariant,
    EmptyBitflag,
    Class(Box<NyarClass>),
    Variant(Box<NyarVariants>),
    Bitflag(Box<NyarBitflags>),
}

#[derive(Debug, Clone)]
pub struct NyarClass {
    name: String,
    base: Vec<Arc<NyarClass>>,
    properties: HashMap<String, NyarProperty>,
    methods: HashMap<String, NyarProperty>,
}

pub struct NyarVariants {}

pub struct NyarBitflags {}

impl Default for NyarClass {
    fn default() -> Self {
        Self { name: String::from("Object"), base: vec![], properties: Default::default(), methods: Default::default() }
    }
}

impl NyarClass {
    pub fn inherit() {}
}
