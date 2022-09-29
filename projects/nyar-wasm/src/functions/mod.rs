use crate::{
    helpers::{IntoWasm, WasmInstruction},
    operations::Operation,
    symbols::WasmExternalName,
    types::WasmType,
    WasmSymbol,
};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use std::{collections::BTreeMap, slice::Iter};
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
#[derive(Debug)]
pub struct FunctionType {
    pub symbol: WasmSymbol,
    pub export: Option<WasmExternalName>,
    pub entry: bool,
    pub input: IndexMap<String, WasmParameter>,
    pub local: BTreeMap<String, WasmParameter>,
    pub output: WasmType,
    pub body: FunctionBody,
    pub span: FileSpan,
}

#[derive(Clone, Debug)]
pub struct WasmParameter {
    pub name: WasmSymbol,
    pub type_hint: WasmType,
    pub span: FileSpan,
}

impl WasmParameter {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<WasmSymbol>,
    {
        Self { name: name.into(), type_hint: WasmType::Any { nullable: true }, span: Default::default() }
    }
    pub fn with_name<S>(self, name: S) -> Self
    where
        S: Into<WasmSymbol>,
    {
        Self { name: name.into(), ..self }
    }
    pub fn with_type(self, type_hint: WasmType) -> Self {
        Self { type_hint, ..self }
    }
}

impl FunctionType {
    pub fn new<S: Into<WasmSymbol>>(name: S) -> Self {
        Self {
            symbol: name.into(),
            export: None,
            entry: false,
            input: IndexMap::default(),
            local: BTreeMap::default(),
            output: WasmType::Tuple(vec![]),
            body: FunctionBody::default(),
            span: Default::default(),
        }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn auto_export(self, export: bool) -> Self {
        Self { export: WasmExternalName::create_by(&self.symbol, export), ..self }
    }
    pub fn with_export<N: Into<WasmExternalName>>(self, export: N) -> Self {
        Self { export: Some(export.into()), ..self }
    }
    pub fn with_entry(self, entry: bool) -> Self {
        Self { entry, ..self }
    }
    pub fn with_inputs<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        for i in inputs {
            self.input.insert(i.name.to_string(), i);
        }
        self
    }
    pub fn with_output<I>(mut self, output: I) -> Self
    where
        I: Into<WasmType>,
    {
        self.output = output.into();
        self
    }
    pub fn with_outputs<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        self.output = WasmType::Tuple(outputs.into_iter().collect());
        self
    }
    pub fn with_locals<I>(mut self, locals: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        for i in locals {
            self.local.insert(i.name.to_string(), i);
        }
        self
    }
    pub fn with_operations<I>(mut self, operations: I) -> Self
    where
        I: IntoIterator<Item = Operation>,
    {
        self.body.codes.extend(operations);
        self
    }
}

#[derive(Debug, Default)]
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

impl From<WasmType> for WasmParameter {
    fn from(value: WasmType) -> Self {
        Self { name: WasmSymbol::new(""), type_hint: value, span: Default::default() }
    }
}
