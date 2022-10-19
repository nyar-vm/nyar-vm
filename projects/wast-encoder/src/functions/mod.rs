use std::{
    fmt::{Display, Formatter, Write},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::{DependencyItem, DependentGraph},
    Identifier, ResolveDependencies, WasiModule, WasiType,
};

mod arithmetic;

///         (export "[method]output-stream.blocking-write-and-flush"
///             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
///         )
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalFunction {
    pub name: Identifier,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
    pub inputs: Vec<WasiParameter>,
    pub output: Option<WasiType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WasiParameter {
    pub name: Arc<str>,
    pub wasi_name: Arc<str>,

    pub r#type: WasiType,
}

impl ExternalFunction {
    pub fn new<S, M>(wasi_module: M, wasi_name: &str, name: S) -> Self
    where
        S: Into<Identifier>,
        M: Into<WasiModule>,
    {
        Self {
            name: name.into(),
            wasi_module: wasi_module.into(),
            wasi_name: wasi_name.to_string(),
            inputs: vec![],
            output: None,
        }
    }
    // pub fn constructor<S, M>(wasi_module: M, wasi_class: &str, name: S) -> Self
    // where
    //     S: Into<Arc<str>>,
    //     M: Into<WasiModule>,
    // {
    //     let wasi_name = format!("[constructor]{}", wasi_class);
    //     Self { name: name.into(), wasi_module: wasi_module.into(), wasi_name, inputs: vec![], output: None }
    // }
    // pub fn static_method<S, M>(wasi_module: M, name: S, wasi_class: &str, wasi_name: &str) -> Self
    // where
    //     S: Into<Arc<str>>,
    //     M: Into<WasiModule>,
    // {
    //     let wasi_name = format!("[static]{}.{}", wasi_class, wasi_name);
    //     Self { name: name.into(), wasi_name, inputs: vec![], output: None }
    // }
    // pub fn method<S>(name: S, wasi_class: &str, wasi_name: &str) -> Self
    // where
    //     S: Into<Arc<str>>,
    //     M: Into<WasiModule>,
    // {
    //     let wasi_name = format!("[method]{}.{}", wasi_class, wasi_name);
    //     Self { name: name.into(), wasi_name, inputs: vec![], output: None }
    // }
    // pub fn destructor<S, M>(name: S, wasi_class: &str) -> Self
    // where
    //     S: Into<Arc<str>>,
    //     M: Into<WasiModule>,
    // {
    //     let wasi_name = format!("[resource-drop]{}", wasi_class);
    //     Self { name: name.into(), wasi_name, inputs: vec![], output: None }
    // }
}

impl WasiParameter {
    pub fn new<S>(name: S, r#type: WasiType) -> Self
    where
        S: Into<Arc<str>>,
    {
        let wasi_name = name.into();
        Self { name: wasi_name.clone(), wasi_name, r#type }
    }
}

impl Display for ExternalFunction {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", self.name)?;
        for (i, input) in self.inputs.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?
            }
            match input.name.as_ref().eq("self") {
                true => f.write_str("self")?,
                false => write!(f, "{}: {:#}", input.name, input.r#type)?,
            }
        }
        f.write_char(')')
    }
}

impl ResolveDependencies for ExternalFunction {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.name.clone(), WasiType::External(Box::new(self.clone())));
    }

    fn collect_wasi_types(&self, dict: &mut DependentGraph) {
        self.inputs.iter().for_each(|input| input.r#type.collect_wasi_types(dict));
        self.output.iter().for_each(|output| output.collect_wasi_types(dict));
        dict.insert_with_dependency(
            DependencyItem::Item(WasiType::External(Box::new(self.clone()))),
            DependencyItem::Module(self.wasi_module.clone()),
        );
        dict.finalize_buffer(WasiType::External(Box::new(self.clone())))
    }
}
