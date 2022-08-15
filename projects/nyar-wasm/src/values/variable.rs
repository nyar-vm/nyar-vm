use super::*;

pub struct WasmVariable {
    pub symbol: WasmSymbol,
    pub constant: bool,
    pub export: bool,
    pub r#type: WasmType,
    pub value: WasmValue,
    pub span: FileSpan,
}

impl Default for WasmVariable {
    fn default() -> Self {
        Self {
            symbol: WasmSymbol::new("<anonymous>"),
            constant: false,
            export: false,
            r#type: WasmType::U8,
            value: WasmValue::Array,
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
    pub fn mutable(&self) -> bool {
        !self.constant
    }

    pub fn with_public(self) -> Self {
        Self { export: true, ..self }
    }

    pub fn value(&self) -> &WasmValue {
        &self.value
    }
}
