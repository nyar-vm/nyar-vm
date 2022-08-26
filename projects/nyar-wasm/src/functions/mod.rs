use crate::{
    helpers::{IntoWasm, WasmInstruction},
    operations::Operation,
    symbols::WasmExportName,
    types::WasmType,
    WasmSymbol,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use std::slice::Iter;
use wast::{
    component::{
        CanonLift, CanonLower, ComponentFunctionParam, ComponentFunctionResult, ComponentFunctionType, ComponentTypeUse,
        CoreFunc, CoreFuncKind, Func, FuncKind, Start,
    },
    core::{Expression, TypeUse, ValType},
    token::{Id, NameAnnotation, Span},
};
pub mod codegen;

/// `function`
pub struct FunctionType {
    pub symbol: WasmSymbol,
    pub export: WasmExportName,
    pub entry: bool,
    pub input: IndexMap<String, ParameterType>,
    pub output: Vec<WasmType>,
    pub body: FunctionBody,
    pub span: FileSpan,
}

pub struct ParameterType {
    pub name: WasmSymbol,
    pub type_hint: WasmType,
    pub span: FileSpan,
}

impl ParameterType {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<WasmSymbol>,
    {
        Self { name: name.into(), type_hint: WasmType::Any { nullable: true }, span: Default::default() }
    }
    pub fn with_type(self, type_hint: WasmType) -> Self {
        Self { type_hint, ..self }
    }
}

impl FunctionType {
    pub fn new(path: WasmSymbol) -> Self {
        Self {
            symbol: path,
            export: WasmExportName::default(),
            entry: false,
            input: IndexMap::default(),
            output: vec![],
            body: FunctionBody::default(),
            span: Default::default(),
        }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn with_export(self, export: bool) -> Self {
        let export = match export {
            true => WasmExportName::create(self.symbol.clone()),
            false => WasmExportName::default(),
        };
        Self { export, ..self }
    }
    pub fn with_entry(self) -> Self {
        Self { entry: true, ..self }
    }
    pub fn with_inputs<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = ParameterType>,
    {
        for i in inputs {
            self.input.insert(i.name.to_string(), i);
        }
        self
    }
    pub fn with_outputs<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmType>,
    {
        self.output = outputs.into_iter().collect();
        self
    }
    pub fn with_operations<I>(mut self, operations: I) -> Self
    where
        I: IntoIterator<Item = Operation>,
    {
        self.body.codes = operations.into_iter().collect();
        self
    }
}

#[derive(Default)]
pub struct FunctionBody {
    codes: Vec<Operation>,
}

impl<'i> IntoIterator for &'i FunctionBody {
    type Item = &'i Operation;
    type IntoIter = Iter<'i, Operation>;

    fn into_iter(self) -> Self::IntoIter {
        self.codes.iter()
    }
}
