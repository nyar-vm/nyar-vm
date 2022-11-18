use super::*;

impl Hash for WasiValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Boolean(v) => {
                "Boolean".hash(state);
                v.hash(state)
            }
            Self::Integer8(v) => {
                "Integer8".hash(state);
                v.hash(state)
            }
            Self::Integer16(v) => {
                "Integer16".hash(state);
                v.hash(state)
            }
            Self::Integer32(v) => {
                "Integer32".hash(state);
                v.hash(state)
            }
            Self::Integer64(v) => {
                "Integer64".hash(state);
                v.hash(state)
            }
            Self::Unsigned8(v) => {
                "Unsigned8".hash(state);
                v.hash(state)
            }
            Self::Unsigned16(v) => {
                "Unsigned16".hash(state);
                v.hash(state)
            }
            Self::Unsigned32(v) => {
                "Unsigned32".hash(state);
                v.hash(state)
            }
            Self::Unsigned64(v) => {
                "Unsigned64".hash(state);
                v.hash(state)
            }
            Self::Float32(v) => {
                "Float32".hash(state);
                v.to_le_bytes().hash(state)
            }
            Self::Float64(v) => {
                "Float64".hash(state);
                v.to_le_bytes().hash(state)
            }
            Self::DynamicArray(v) => {
                "DynamicArray".hash(state);
                v.hash(state)
            }
        }
    }
}

impl Eq for WasiValue {}

impl PartialEq<Self> for WasiValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Boolean(lhs), Self::Boolean(rhs)) => lhs.eq(rhs),
            (Self::Integer8(lhs), Self::Integer8(rhs)) => lhs.eq(rhs),
            (Self::Integer16(lhs), Self::Integer16(rhs)) => lhs.eq(rhs),
            (Self::Integer32(lhs), Self::Integer32(rhs)) => lhs.eq(rhs),
            (Self::Integer64(lhs), Self::Integer64(rhs)) => lhs.eq(rhs),
            (Self::Unsigned8(lhs), Self::Unsigned8(rhs)) => lhs.eq(rhs),
            (Self::Unsigned16(lhs), Self::Unsigned16(rhs)) => lhs.eq(rhs),
            (Self::Unsigned32(lhs), Self::Unsigned32(rhs)) => lhs.eq(rhs),
            (Self::Unsigned64(lhs), Self::Unsigned64(rhs)) => lhs.eq(rhs),
            // (Self::Float32(lhs), Self::Float32(rhs)) => lhs.eq(rhs),
            // (Self::Float64(lhs), Self::Float64(rhs)) => lhs.eq(rhs),
            // (Self::DynamicArray(lhs), Self::DynamicArray(rhs)) => lhs.eq(rhs),
            _ => false,
        }
    }
}

impl Ord for WasiValue {
    fn cmp(&self, other: &Self) -> Ordering {
        todo!()
    }
}

impl PartialOrd<Self> for WasiValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Boolean(lhs), Self::Boolean(rhs)) => lhs.partial_cmp(rhs),
            (Self::Integer8(lhs), Self::Integer8(rhs)) => lhs.partial_cmp(rhs),
            (Self::Integer16(lhs), Self::Integer16(rhs)) => lhs.partial_cmp(rhs),
            (Self::Integer32(lhs), Self::Integer32(rhs)) => lhs.partial_cmp(rhs),
            (Self::Integer64(lhs), Self::Integer64(rhs)) => lhs.partial_cmp(rhs),
            (Self::Unsigned8(lhs), Self::Unsigned8(rhs)) => lhs.partial_cmp(rhs),
            (Self::Unsigned16(lhs), Self::Unsigned16(rhs)) => lhs.partial_cmp(rhs),
            (Self::Unsigned32(lhs), Self::Unsigned32(rhs)) => lhs.partial_cmp(rhs),
            (Self::Unsigned64(lhs), Self::Unsigned64(rhs)) => lhs.partial_cmp(rhs),
            // (Self::Float32(lhs), Self::Float32(rhs)) => lhs.eq(rhs),
            // (Self::Float64(lhs), Self::Float64(rhs)) => lhs.eq(rhs),
            // (Self::DynamicArray(lhs), Self::DynamicArray(rhs)) => lhs.eq(rhs),
            _ => None,
        }
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
