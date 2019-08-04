
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
