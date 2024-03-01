use std::{
    fmt::{Display, Formatter, Write},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::DependentGraph,
    DependenciesTrace,
    helpers::{AliasExport, LowerFunction, TypeReferenceInput, TypeReferenceOutput}, Identifier, WasiModule, WasiType, WastEncoder,
};

mod arithmetic;
mod display;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalFunction {
    pub symbol: Identifier,
    pub wasi_module: WasiModule,
    pub wasi_name: String,
    pub inputs: Vec<WasiParameter>,
    pub output: Option<WasiType>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
            symbol: name.into(),
            wasi_module: wasi_module.into(),
            wasi_name: wasi_name.to_string(),
            inputs: vec![],
            output: None,
        }
    }
}

impl WasiParameter {
    pub fn new<S, T>(name: S, r#type: T) -> Self
    where
        S: Into<Arc<str>>,
        T: Into<WasiType>,
    {
        let wasi_name = name.into();
        Self { name: wasi_name.clone(), wasi_name, r#type: r#type.into() }
    }
}
impl AliasExport for ExternalFunction {
    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        let id = self.symbol.wasi_id();
        let name = self.wasi_name.as_str();
        write!(w, "(alias export ${module} \"{name}\" (func {id}))")
    }
}

impl LowerFunction for ExternalFunction {
    fn lower_function<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(core func {} (canon lower", self.symbol.wasi_id())?;
        w.indent();
        w.newline()?;
        write!(w, "(func ${} \"{}\")", self.wasi_module, self.wasi_name)?;
        w.newline()?;
        write!(w, "(memory $memory \"memory\")")?;
        write!(w, "(realloc (func $memory \"realloc\"))")?;
        w.newline()?;
        write!(w, "string-encoding=utf8")?;
        w.dedent(2);
        Ok(())
    }

    //         (type (func (param i64 i32)))
    //         (import "wasi:random/random@0.2.0" "get-random-bytes" (func $wasi:random/random@0.2.0:get-random-bytes (;0;) (type 0)))
    fn lower_import<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(import \"{}\" \"{}\" (func {}", self.wasi_module, self.wasi_name, self.symbol.wasi_id())?;
        for input in &self.inputs {
            w.write_str(" ")?;
            input.lower_input(w)?;
        }
        match &self.output {
            Some(s) => {
                w.write_str(" ")?;
                s.lower_output(w)?;
            }
            None => {}
        }
        w.write_str("))")
    }
}

impl DependenciesTrace for ExternalFunction {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::External(Box::new(self.clone())));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.inputs.iter().for_each(|input| input.r#type.collect_wasi_types(dict, collected));
        self.output.iter().for_each(|output| output.collect_wasi_types(dict, collected));
    }
}
