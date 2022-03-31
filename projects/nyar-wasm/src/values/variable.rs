use super::*;

pub struct WasmVariable {
    pub symbol: Symbol,
    pub constant: bool,
    pub export: bool,
    pub r#type: NyarType,
    pub value: NyarValue,
    pub span: FileSpan,
}

impl Default for WasmVariable {
    fn default() -> Self {
        Self {
            symbol: Symbol::new("<anonymous>"),
            constant: false,
            export: false,
            r#type: NyarType::U8,
            value: NyarValue::Array,
            span: Default::default(),
        }
    }
}

impl<'a, 'i> WasmOutput<'a, Expression<'i>> for WasmVariable
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Expression<'i> {
        Expression { instrs: Box::from(vec![self.value.as_wast()]) }
    }
}

impl WasmVariable {
    /// Create a new [`i32`] value
    pub fn i32(name: Symbol, value: i32) -> Self {
        Self { symbol: name, value: NyarValue::I32(value), r#type: NyarType::I32, ..Self::default() }
    }
    pub fn i64(name: Symbol, value: i64) -> Self {
        Self { symbol: name, value: NyarValue::I64(value), r#type: NyarType::I64, ..Self::default() }
    }
    pub fn f32(name: Symbol, value: f32) -> Self {
        Self { symbol: name, value: NyarValue::F32(value), r#type: NyarType::F32, ..Self::default() }
    }
    pub fn f64(name: Symbol, value: f64) -> Self {
        Self { symbol: name, value: NyarValue::F64(value), r#type: NyarType::F64, ..Self::default() }
    }
    pub fn function(name: Symbol) -> Self {
        Self { symbol: name.clone(), value: NyarValue::Function(name), ..Self::default() }
    }
    pub fn mutable(&self) -> bool {
        !self.constant
    }

    pub fn with_public(self) -> Self {
        Self { export: true, ..self }
    }

    pub fn value(&self) -> &NyarValue {
        &self.value
    }
}
