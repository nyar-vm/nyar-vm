use super::*;

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

pub struct NyarProperty {
    // typing: Option<Typing>,
    default: Rc<NyarClass>,
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

pub struct NyarClass {
    name: String,
    base: Vec<Rc<NyarClass>>,
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
