use crate::typing::Typing;
use bitflags::bitflags;
use std::{
    collections::{BTreeMap, HashMap},
    rc::Rc,
    task::Context,
};

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

#[rustfmt::skip]
bitflags! {
    /// ## Access control character
    /// | Scopes    | curr module | sub module | curr package | other package |
    /// | :-------- | :---------: | :--------: | :----------: | :-----------: |
    /// | public     |      √     |     √      |      √       |       √       |
    /// | internal   |      √     |     √      |      √       |       ×       |
    /// | private    |      √     |     √      |      ×       |       ×       |
    /// | restricted |      √     |     ×      |      ×       |       ×       |
    ///
    #[allow(non_upper_case_globals)]
    pub struct NyarReadWrite: u8 {
        const SelfRead      = 0b00000001;
        const SelfWrite     = 0b00000010;
        const ModuleRead    = 0b00000100;
        const ModuleWrite   = 0b00001000;
        const PackageRead   = 0b00010000;
        const PackageWrite  = 0b00100000;
        const GlobalRead    = 0b01000000;
        const GlobalWrite   = 0b10000000;
        /// self modify
        const Restricted = Self::SelfRead.bits | Self::SelfWrite.bits;
        ///
        const Private = Self::ModuleRead.bits | Self::ModuleWrite.bits | Self::Restricted.bits;
        /// inside
        const Internal = Self::PackageRead.bits | Self::PackageWrite.bits | Self::Private.bits;
        ///
        const Public = Self::GlobalRead.bits | Self::GlobalWrite.bits | Self::Internal.bits;
    }
}

pub struct NyarProperty {
    // typing: Option<Typing>,
    default: Rc<NyarClass>,
    visibility: NyarReadWrite,
}

impl Default for NyarReadWrite {
    fn default() -> Self {
        Self::Public
    }
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
        Self { implicit_self: false, index_system: NyarIndexSystem::OrdinalSystem, uniform_function_call_syntax: true }
    }
}

pub struct NameSpace {
    name: String,
    base: Option<Box<NameSpace>>,
    ctx: NyarContext,
    classes: HashMap<String, Rc<NyarClass>>,
}
