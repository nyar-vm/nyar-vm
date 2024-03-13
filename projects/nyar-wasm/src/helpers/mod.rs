use std::fmt::Write;

use crate::{encoder::WastEncoder, WasiModule, WasiType, WasiValue};

pub trait ToWasiType {
    fn to_wasi_type(&self) -> WasiType;
}

pub trait ToWasiValue {
    fn to_wasi_value(&self) -> WasiValue;
}

/// Mark for type who can import to the component instance
pub(crate) trait AliasOuter {
    fn alias_outer<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait AliasExport {
    fn alias_export<W: Write>(&self, w: &mut WastEncoder<W>, module: &WasiModule) -> std::fmt::Result;
}

/// Mark for type who can define in component section
pub(crate) trait ComponentDefine {
    fn component_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait LowerFunction {
    fn lower_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn wasm_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait TypeDefinition {
    fn upper_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn lower_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait TypeReference {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn lower_type_inner<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait TypeReferenceInput {
    fn upper_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn lower_input<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait TypeReferenceOutput {
    fn upper_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn lower_output<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait EmitDefault {
    /// Emit default instruction for the value
    fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait EmitConstant {
    /// Emit constant instruction for the value
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}
