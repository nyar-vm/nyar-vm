use std::{
    fmt::{Display, Formatter, Write},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::DependentGraph,
    helpers::{AliasExport, LowerFunction, TypeReferenceInput, TypeReferenceOutput},
    operations::WasiInstruction,
    DependenciesTrace, Identifier, WasiModule, WasiType, WastEncoder,
};

mod arithmetic;
mod display;

/// The type of external WASI function
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiFunction {
    /// The symbol of the function in source language
    pub symbol: Identifier,
    /// The input parameters of the function
    pub inputs: Vec<WasiParameter>,
    /// The output parameter of the function
    pub output: Vec<WasiParameter>,
    pub body: WasiFunctionBody,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiFunctionBody {
    External {
        /// The external module name registered in WASI host
        wasi_module: WasiModule,
        /// The external function name registered in WASI host
        wasi_name: String,
    },
    Normal {
        bytecodes: Vec<WasiInstruction>,
    },
}

/// The type of WASI parameter
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WasiParameter {
    /// The name of the parameter in source language
    pub name: Arc<str>,
    /// The name of the parameter in WASI host
    pub wasi_name: Arc<str>,
    /// The type of the parameter
    pub r#type: WasiType,
}

impl WasiFunction {
    /// Create a new external function type with the given symbol and WASI module
    pub fn external<S, M>(wasi_module: M, wasi_name: &str, name: S) -> Self
    where
        S: Into<Identifier>,
        M: Into<WasiModule>,
    {
        Self {
            symbol: name.into(),
            inputs: vec![],
            output: vec![],
            body: WasiFunctionBody::External { wasi_module: wasi_module.into(), wasi_name: wasi_name.to_string() },
        }
    }
}

impl WasiParameter {
    /// Create a new WASI parameter with the given name and type
    pub fn new<S, T>(name: S, r#type: T) -> Self
    where
        S: Into<Arc<str>>,
        T: Into<WasiType>,
    {
        let wasi_name = name.into();
        Self { name: wasi_name.clone(), wasi_name, r#type: r#type.into() }
    }
}
impl AliasExport for WasiFunction {
    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result {
        let id = self.symbol.wasi_id();
        match &self.body {
            WasiFunctionBody::External { wasi_name, .. } => write!(w, "(alias export ${module} \"{wasi_name}\" (func {id}))")?,
            WasiFunctionBody::Normal { .. } => {}
        }
        Ok(())
    }
}

impl LowerFunction for WasiFunction {
    fn lower_function<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(core func {} (canon lower", self.symbol.wasi_id())?;
        w.indent();
        w.newline()?;
        match &self.body {
            WasiFunctionBody::External { wasi_module, wasi_name } => {
                write!(w, "(func ${} \"{}\")", wasi_module, wasi_name)?;
            }
            WasiFunctionBody::Normal { .. } => {}
        }

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
        match &self.body {
            WasiFunctionBody::External { wasi_module, wasi_name } => {
                write!(w, "(import \"{}\" \"{}\" (func {}", wasi_module, wasi_name, self.symbol.wasi_id())?;
            }
            WasiFunctionBody::Normal { .. } => {}
        }
        for input in &self.inputs {
            w.write_str(" ")?;
            input.lower_input(w)?;
        }
        for output in &self.inputs {
            w.write_str(" ")?;
            output.lower_input(w)?;
        }
        w.write_str("))")
    }
}

impl DependenciesTrace for WasiFunction {
    fn define_language_types(&self, dict: &mut DependentGraph) {
        dict.types.insert(self.symbol.clone(), WasiType::Function(Box::new(self.clone())));
    }

    fn collect_wasi_types<'a, 'i>(&'a self, dict: &'i DependentGraph, collected: &mut Vec<&'i WasiType>)
    where
        'a: 'i,
    {
        self.inputs.iter().for_each(|input| input.r#type.collect_wasi_types(dict, collected));
        self.output.iter().for_each(|output| output.r#type.collect_wasi_types(dict, collected));
    }
}
