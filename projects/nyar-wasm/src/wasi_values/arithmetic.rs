use super::*;

impl Hash for WasiValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            WasiValue::Boolean(v) => {
                "Boolean".hash(state);
                v.hash(state)
            }
            WasiValue::Integer8(v) => {}
            WasiValue::Integer16(v) => {}
            WasiValue::Integer32(v) => {}
            WasiValue::Integer64(v) => {}
            WasiValue::Unsigned8(v) => {}
            WasiValue::Unsigned16(v) => {}
            WasiValue::Unsigned32(v) => {}
            WasiValue::Unsigned64(v) => {}
            WasiValue::Float32(v) => {}
            WasiValue::Float64(v) => {}
            WasiValue::DynamicArray { .. } => {}
        }
    }
}

impl Eq for WasiValue {}

impl PartialEq<Self> for WasiValue {
    fn eq(&self, other: &Self) -> bool {
        todo!()
    }
}

impl PartialOrd<Self> for WasiValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        todo!()
    }
}

impl Ord for WasiValue {
    fn cmp(&self, other: &Self) -> Ordering {
        todo!()
    }
}

impl ToWasiType for WasiValue {
    fn to_wasi_type(&self) -> WasiType {
        match self {
            Self::Boolean(_) => WasiType::Boolean,
            Self::Integer8(_) => WasiType::Integer8 { signed: true },
            Self::Integer16(_) => WasiType::Integer8 { signed: true },
            Self::Integer32(_) => WasiType::Integer8 { signed: true },
            Self::Integer64(_) => WasiType::Integer8 { signed: true },
            Self::Unsigned8(_) => WasiType::Integer8 { signed: false },
            Self::Unsigned16(_) => WasiType::Integer8 { signed: false },
            Self::Unsigned32(_) => WasiType::Integer8 { signed: false },
            Self::Unsigned64(_) => WasiType::Integer8 { signed: false },
            Self::Float32(_) => WasiType::Float32,
            Self::Float64(_) => WasiType::Float64,
            Self::DynamicArray(v) => v.to_wasi_type(),
        }
    }
}
