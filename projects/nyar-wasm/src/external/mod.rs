use crate::{
    helpers::{IntoWasm, WasmName},
    symbols::WasmExternalName,
    WasmParameter, WasmSymbol, WasmType,
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
    pub external_package: WasmExternalName,
    /// External name of the type
    pub external_name: WasmSymbol,
    pub local_name: WasmSymbol,
    pub inputs: Vec<WasmParameter>,
    pub output: WasmType,
}

impl AddAssign<WasmParameter> for ImportFunction {
    fn add_assign(&mut self, rhs: WasmParameter) {
        self.inputs.push(rhs)
    }
}

impl ImportFunction {
    pub fn new<M: Into<WasmExternalName>, F: Into<WasmSymbol>>(module: M, function: F) -> ImportFunction {
        let external_name = function.into();
        Self {
            external_package: module.into(),
            external_name: external_name.clone(),
            local_name: external_name,
            inputs: vec![],
            output: WasmType::Tuple(vec![]),
        }
    }
    pub fn name(&self) -> &str {
        self.local_name.as_ref()
    }
    pub fn function_id(&self) -> &str {
        // SAFETY: StringPool will deallocate this string when there are no more references to it
        unsafe {
            let cache = string_pool::String::from(format!("{}:{}", self.external_package, self.external_name));
            &*(cache.as_str() as *const str)
        }
    }

    pub fn with_local<S: Into<WasmSymbol>>(self, alias: S) -> Self {
        Self { local_name: alias.into(), ..self }
    }
    pub fn with_input<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        self.inputs.extend(inputs);
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
}
