use super::*;

impl From<WasiArrayType> for WasiType {
    fn from(value: WasiArrayType) -> Self {
        Self::Array(Box::new(value))
    }
}
