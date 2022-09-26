use crate::{
    helpers::{IntoWasm, WasmName},
    symbols::WasmExternalName,
    WasmParameter, WasmSymbol,
};
use std::ops::AddAssign;
use wast::{
    core::{Import, ItemKind, TypeUse},
    token::Span,
};

mod codegen;

/// `@ffi("org:project/module@version", "field")`
#[derive(Debug)]
pub struct ImportFunction {
    /// External path of the type
    pub external: WasmExternalName,
    /// Local name of the type
    pub local: WasmSymbol,
    pub alias: Option<WasmSymbol>,
    pub inputs: Vec<WasmParameter>,
    pub outputs: Vec<WasmParameter>,
}

impl AddAssign<WasmSymbol> for ImportFunction {
    fn add_assign(&mut self, rhs: WasmSymbol) {
        self.alias = Some(rhs)
    }
}

impl AddAssign<WasmParameter> for ImportFunction {
    fn add_assign(&mut self, rhs: WasmParameter) {
        self.inputs.push(rhs)
    }
}

impl ImportFunction {
    pub fn new<M: Into<WasmExternalName>, F: Into<WasmSymbol>>(module: M, function: F) -> ImportFunction {
        Self { external: module.into(), local: function.into(), alias: None, inputs: vec![], outputs: vec![] }
    }
    pub fn name(&self) -> &str {
        match &self.alias {
            None => self.local.as_ref(),
            Some(s) => s.as_ref(),
        }
    }
    pub fn function_id(&self) -> &str {
        // SAFETY: StringPool will deallocate this string when there are no more references to it
        unsafe {
            let cache = string_pool::String::from(format!("{}/{}", self.external, self.local));
            &*(cache.as_str() as *const str)
        }
    }

    pub fn with_alias<S: Into<WasmSymbol>>(self, alias: S) -> Self {
        Self { alias: Some(alias.into()), ..self }
    }
    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        self.inputs.extend(inputs);
        self
    }
    pub fn with_output<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        self.outputs.extend(outputs);
        self
    }
}
