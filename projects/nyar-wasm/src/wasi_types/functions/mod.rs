use std::{
    fmt::{Display, Formatter, Write},
    ops::AddAssign,
    sync::Arc,
};

use crate::{
    dag::DependentGraph,
    helpers::{AliasExport, LowerFunction, TypeReferenceInput},
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
        wasi_name: Arc<str>,
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
    pub fn external(wasi_module: &WasiModule, wasi_name: &Arc<str>, name: &Identifier) -> Self {
        Self {
            symbol: name.clone(),
            inputs: vec![],
            output: vec![],
            body: WasiFunctionBody::External { wasi_module: wasi_module.clone(), wasi_name: wasi_name.clone() },
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
