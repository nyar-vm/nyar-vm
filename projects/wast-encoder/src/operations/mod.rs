use crate::{encoder::WastEncoder, Identifier, WasiType, WasiValue};
use std::fmt::Write;

pub(crate) trait Emit {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait EmitValue {
    fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub enum WasiInstruction {
    /// Create the default value for a given type
    Default(WasiType),
    /// Create a constant value
    Constant(WasiValue),
    Conversion {
        from: WasiType,
        into: WasiType,
    },
    GetField,
    SetField,
    CallFunction {
        symbol: Identifier,
        parameters: Vec<WasiValue>,
    },
    Drop {
        objects: usize,
    },
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn emit_instructions(&mut self, instruction: &[WasiInstruction]) -> std::fmt::Result {
        for i in instruction {
            match i {
                WasiInstruction::Default(v) => v.emit_default(self)?,
                WasiInstruction::Constant(v) => {
                    todo!()
                }
                WasiInstruction::Conversion { from, into } => {
                    todo!()
                }
                WasiInstruction::GetField => {
                    todo!()
                }
                WasiInstruction::SetField => {
                    todo!()
                }
                WasiInstruction::CallFunction { symbol, parameters } => {
                    todo!()
                }
                WasiInstruction::Drop { objects } => {
                    todo!()
                }
            }
        }
        Ok(())
    }
}
impl WasiType {
    pub fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            WasiType::Boolean => {
                write!(w, "(i32.const 0)")
            }
            WasiType::Integer8 { .. } => {
                write!(w, "(i32.const 0)")
            }
            WasiType::Integer16 { .. } => {
                write!(w, "(i32.const 0)")
            }
            WasiType::Integer32 { .. } => {
                write!(w, "(i32.const 0)")
            }
            WasiType::Integer64 { .. } => {
                write!(w, "(i64.const 0)")
            }
            WasiType::Float32 => {
                write!(w, "(f32.const 0)")
            }
            WasiType::Float64 => {
                write!(w, "(f64.const 0)")
            }
            WasiType::Option { .. } => {
                todo!()
            }
            WasiType::Result { .. } => {
                todo!()
            }
            WasiType::Resource(_) => {
                todo!()
            }
            WasiType::Record(_) => {
                todo!()
            }
            WasiType::Variant(_) => {
                todo!()
            }
            WasiType::TypeHandler { .. } => {
                todo!()
            }
            WasiType::TypeQuery { .. } => {
                todo!()
            }
            WasiType::Array(_) => {
                todo!()
            }
            WasiType::External(_) => {
                todo!()
            }
        }
    }
}

impl WasiValue {
    pub fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        Self::default().emit_constant(w)
    }

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
            Self::Array { .. } => {
                todo!()
            }
        }
        Ok(())
    }
}
