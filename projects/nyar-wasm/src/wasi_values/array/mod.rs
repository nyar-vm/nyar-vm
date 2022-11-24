use super::*;
use crate::Identifier;

/// An array with dynamic value
#[derive(Debug, Clone, Hash)]
pub struct ArrayValue {
    /// The type of the array
    pub r#type: WasiArrayType,
    /// The values of the array
    pub values: Vec<WasiValue>,
}

#[derive(Debug, Clone)]
pub struct ArrayData {
    pub symbol: Identifier,
    pub values: Vec<u8>,
}

impl ToWasiType for ArrayValue {
    fn to_wasi_type(&self) -> WasiType {
        WasiType::Array(Box::new(self.r#type.clone()))
    }
}

impl EmitConstant for ArrayValue {
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        todo!()
    }
}
