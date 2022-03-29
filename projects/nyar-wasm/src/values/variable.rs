use super::*;

pub struct WasmVariable {
    pub symbol: Symbol,
    pub constant: bool,
    pub export: bool,
    pub value: NyarValue,
    pub span: FileSpan,
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
        Self { symbol: name, value: NyarValue::I32(value), constant: false, export: false, span: Default::default() }
    }
    pub fn i64(name: Symbol, value: i32) -> Self {
        Self { symbol: name, value: NyarValue::I32(value), constant: false, export: false, span: Default::default() }
    }
    pub fn f32(name: Symbol, value: f32) -> Self {
        Self { symbol: name, value: NyarValue::F32(value), constant: false, export: false, span: Default::default() }
    }
    pub fn f64(name: Symbol, value: f32) -> Self {
        Self { symbol: name, value: NyarValue::F32(value), constant: false, export: false, span: Default::default() }
    }
    pub fn function(name: Symbol) -> Self {
        Self {
            symbol: name.clone(),
            value: NyarValue::Function(name),
            constant: false,
            export: false,
            span: Default::default(),
        }
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
