use crate::{encoder::WastEncoder, WasiArrayType};
use std::fmt::Write;

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

impl WasiValue {
    pub fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean(v) => write!(w, "(i32.const {})", if *v { 1 } else { 0 })?,
            Self::Integer8(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer16(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer32(v) => write!(w, "(i32.const {})", v)?,
            Self::Integer64(v) => write!(w, "(i64.const {})", v)?,
            Self::Unsigned8(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned16(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned32(v) => write!(w, "(i32.const {})", v)?,
            Self::Unsigned64(v) => write!(w, "(i64.const {})", v)?,
            Self::Float32(v) => write!(w, "(f32.const {})", v)?,
            Self::Float64(v) => write!(w, "(f64.const {})", v)?,
            Self::DynamicArray { .. } => {
                todo!()
            }
        }
        Ok(())
    }
}
