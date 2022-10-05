use crate::{
    helpers::{IntoWasm, WasmName},
    symbols::WasmExternalName,
    FunctionSignature, WasmParameter, WasmSymbol, WasmType,
};
use wast::{
    core::{ItemKind, TypeUse},
    token::Span,
};

mod codegen;

/// `@ffi("org:project/module@version", "field")`
#[derive(Debug)]
pub struct ExternalFunctionType {
    /// External path of the type
    pub external_package: WasmExternalName,
    /// External name of the type
    pub external_name: WasmSymbol,
    pub local_name: WasmSymbol,
    pub signature: FunctionSignature,
}

impl FunctionSignature {
    pub fn with_input<I>(mut self, input: I) -> Self
    where
        I: Into<WasmParameter>,
    {
        self.inputs.push(input.into());
        self
    }

    pub fn with_inputs<I>(mut self, inputs: I) -> Self
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

impl ExternalFunctionType {
    pub fn new<M: Into<WasmExternalName>, F: Into<WasmSymbol>>(module: M, function: F) -> ExternalFunctionType {
        let external_name = function.into();
        Self {
            external_package: module.into(),
            external_name: external_name.clone(),
            local_name: external_name,
            signature: Default::default(),
        }
    }
    pub fn name(&self) -> &str {
        self.local_name.as_ref()
    }
    pub fn function_id(&self) -> &str {
        // SAFETY: StringPool will deallocate this string when there are no more references to it
        // unsafe {
        //     let cache = string_pool::String::from(format!("{}:{}", self.external_package, self.external_name));
        //     &*(cache.as_str() as *const str)
        // }
        self.local_name.as_ref()
    }

    pub fn with_local_name<S: Into<WasmSymbol>>(self, alias: S) -> Self {
        Self { local_name: alias.into(), ..self }
    }
    pub fn with_input<I>(self, inputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        Self { signature: self.signature.with_inputs(inputs), ..self }
    }
    pub fn with_output<I>(self, output: I) -> Self
    where
        I: Into<WasmType>,
    {
        Self { signature: self.signature.with_output(output), ..self }
    }
    pub fn with_outputs<I>(self, outputs: I) -> Self
    where
        I: IntoIterator<Item = WasmParameter>,
    {
        Self { signature: self.signature.with_outputs(outputs), ..self }
    }
}
