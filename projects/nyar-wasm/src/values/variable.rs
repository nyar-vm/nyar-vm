use super::*;
use crate::symbols::WasmExportName;

#[derive(Debug)]
pub struct WasmVariable {
    pub symbol: WasmSymbol,
    pub mutable: bool,
    pub export: WasmExportName,
    pub r#type: WasmType,
    pub value: WasmValue,
    pub span: FileSpan,
}

impl Default for WasmVariable {
    fn default() -> Self {
        Self {
            symbol: WasmSymbol::new("<anonymous>"),
            mutable: false,
            export: WasmExportName::default(),
            r#type: WasmType::U8,
            value: WasmValue::Any,
            span: Default::default(),
        }
    }
}

impl<'a, 'i> IntoWasm<'a, Expression<'i>> for WasmVariable
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Expression<'i> {
        Expression { instrs: Box::from(vec![self.value.as_wast()]) }
    }
}

impl WasmVariable {
    /// Create a new [`i32`] value
    pub fn i32<S: Into<WasmSymbol>>(name: S, value: i32) -> Self {
        Self { symbol: name.into(), value: WasmValue::I32(value), r#type: WasmType::I32, ..Self::default() }
    }
    pub fn i64<S: Into<WasmSymbol>>(name: S, value: i64) -> Self {
        Self { symbol: name.into(), value: WasmValue::I64(value), r#type: WasmType::I64, ..Self::default() }
    }
    pub fn f32<S: Into<WasmSymbol>>(name: S, value: f32) -> Self {
        Self { symbol: name.into(), value: WasmValue::F32(value), r#type: WasmType::F32, ..Self::default() }
    }
    pub fn f64<S: Into<WasmSymbol>>(name: S, value: f64) -> Self {
        Self { symbol: name.into(), value: WasmValue::F64(value), r#type: WasmType::F64, ..Self::default() }
    }
    pub fn function(name: WasmSymbol) -> Self {
        Self { symbol: name.clone(), value: WasmValue::Function(name), ..Self::default() }
    }
    pub fn with_export(self, export: bool) -> Self {
        Self { export: WasmExportName::create_by(&self.symbol, export), ..self }
    }
    pub fn with_mutable(self) -> Self {
        Self { mutable: true, ..self }
    }
    pub fn with_immutable(self) -> Self {
        Self { mutable: false, ..self }
    }
}
