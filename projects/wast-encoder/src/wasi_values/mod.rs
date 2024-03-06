use crate::{encoder::WastEncoder, helpers::ToWasiType, WasiArrayType, WasiType};
use std::{borrow::Cow, fmt::Write};

#[derive(Clone)]
pub enum WasiValue {
    Boolean(bool),
    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
    Float32(f32),
    Float64(f64),
    DynamicArray { r#type: WasiArrayType, values: Vec<WasiValue> },
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
            Self::DynamicArray { r#type, .. } => WasiType::Array(Box::new(r#type.clone())),
        }
    }
}

impl WasiValue {
    pub fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean(v) => write!(w, "i32.const {}", if *v { 1 } else { 0 })?,
            Self::Integer8(v) => write!(w, "i32.const {}", v)?,
            Self::Integer16(v) => write!(w, "i32.const {}", v)?,
            Self::Integer32(v) => write!(w, "i32.const {}", v)?,
            Self::Integer64(v) => write!(w, "i64.const {}", v)?,
            Self::Unsigned8(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned16(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned32(v) => write!(w, "i32.const {}", v)?,
            Self::Unsigned64(v) => write!(w, "i64.const {}", v)?,
            Self::Float32(v) => write!(w, "f32.const {}", v)?,
            Self::Float64(v) => write!(w, "f64.const {}", v)?,
            Self::DynamicArray { .. } => {
                todo!()
            }
        }
        Ok(())
    }
}
