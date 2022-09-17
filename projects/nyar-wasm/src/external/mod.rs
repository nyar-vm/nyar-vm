use crate::{
    helpers::{IntoWasm, WasmName},
    ParameterType, WasmSymbol, WasmType,
};
use std::ops::AddAssign;
use wast::{
    core::{Import, ItemKind, ItemSig, TypeUse},
    token::Span,
};

mod codegen;

/// `@ffi("module", "field")`
#[derive(Debug)]
pub struct ExternalType {
    pub module: WasmSymbol,
    pub function: WasmSymbol,
    pub alias: Option<WasmSymbol>,
    pub input: Vec<ParameterType>,
    pub output: Vec<WasmType>,
}

impl AddAssign<WasmSymbol> for ExternalType {
    fn add_assign(&mut self, rhs: WasmSymbol) {
        self.alias = Some(rhs)
    }
}

impl ExternalType {
    pub fn new<M: Into<WasmSymbol>, F: Into<WasmSymbol>>(module: M, function: F) -> ExternalType {
        Self { module: module.into(), function: function.into(), alias: None, input: vec![], output: vec![] }
    }
    pub fn name(&self) -> &str {
        match &self.alias {
            None => self.function.as_ref(),
            Some(s) => s.as_ref(),
        }
    }
    pub fn with_alias<S: Into<WasmSymbol>>(self, alias: S) -> Self {
        Self { alias: Some(alias.into()), ..self }
    }
    pub fn set_alias<S: Into<WasmSymbol>>(&mut self, alias: S) {
        self.alias = Some(alias.into())
    }
    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = ParameterType>,
    {
        self.input.extend(inputs);
        self
    }
    pub fn with_output<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmType>,
    {
        self.output.extend(outputs);
        self
    }
}
