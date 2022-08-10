use crate::{
    helpers::{Id, IndexedIterator, WasmOutput},
    WasmSymbol, WasmType,
};
use indexmap::IndexMap;
use wast::{
    core::{Import, ItemKind, ItemSig, TypeUse},
    token::Span,
};

mod codegen;

/// `@ffi("module", "field")`
pub struct ExternalType {
    pub module: WasmSymbol,
    pub field: WasmSymbol,
    pub alias: Option<WasmSymbol>,
    pub input: Vec<WasmType>,
    pub output: Vec<WasmType>,
}

impl ExternalType {
    pub fn new(module: &str, field: &str) -> ExternalType {
        Self { module: WasmSymbol::new(module), field: WasmSymbol::new(field), alias: None, input: vec![], output: vec![] }
    }
    pub fn name(&self) -> &str {
        match &self.alias {
            None => self.field.as_ref(),
            Some(s) => s.as_ref(),
        }
    }
    pub fn with_alias<S: Into<WasmSymbol>>(self, alias: S) -> Self {
        Self { alias: Some(alias.into()), ..self }
    }

    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmType>,
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
