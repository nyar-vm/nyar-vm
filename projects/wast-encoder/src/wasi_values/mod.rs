use crate::{encoder::WastEncoder, WasiArrayType, WasiType};
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
    pub fn emit_convert<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean(_) => {
                todo!()
            }
            Self::Integer8(_) => {
                todo!()
            }
            Self::Integer16(_) => {
                todo!()
            }
            Self::Integer32(_) => {
                todo!()
            }
            Self::Integer64(_) => {
                todo!()
            }
            Self::Unsigned8(_) => {
                todo!()
            }
            Self::Unsigned16(_) => {
                todo!()
            }
            Self::Unsigned32(_) => {
                todo!()
            }
            Self::Unsigned64(_) => {
                todo!()
            }
            Self::Float32(_) => {
                todo!()
            }
            Self::Float64(_) => {
                todo!()
            }
            Self::DynamicArray { .. } => {
                todo!()
            }
        }
        Ok(())
    }
    pub fn emit_transmute<W: Write>(&self, target: &WasiType, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            Self::Boolean(_) => {
                todo!()
            }
            Self::Integer8(_) => {
                todo!()
            }
            Self::Integer16(_) => {
                todo!()
            }
            Self::Integer32(_) => {
                todo!()
            }
            Self::Integer64(_) => {
                todo!()
            }
            Self::Unsigned8(_) => {
                todo!()
            }
            Self::Unsigned16(_) => {
                todo!()
            }
            Self::Unsigned32(_) => {
                todo!()
            }
            Self::Unsigned64(_) => {
                todo!()
            }
            Self::Float32(v) => match target {
                WasiType::Integer8 { .. } => {}
                WasiType::Integer16 { .. } => {}
                // f32.reinterpret_i32
                WasiType::Integer32 { .. } => write!(w, "(i32.reinterpret_f32 ")?,
                WasiType::Integer64 { .. } => {}
                WasiType::Float32 => {}
                WasiType::Float64 => {}
                WasiType::Option { .. } => {}
                WasiType::Result { .. } => {}
                WasiType::Resource(_) => {}
                WasiType::Record(_) => {}
                WasiType::Variant(_) => {}
                WasiType::TypeHandler(_) => {}
                WasiType::Array(_) => {}
                WasiType::External(_) => {}
                _ => unreachable!("Can't transmute `f32` to {}", target),
            },
            Self::Float64(_) => {
                todo!()
            }
            Self::DynamicArray { .. } => {
                todo!()
            }
        }
        Ok(())
    }
}
