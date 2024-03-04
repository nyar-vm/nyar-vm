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
            WasiType::Integer8 { .. } => {
                todo!()
            }
            WasiType::Integer16 { .. } => {
                todo!()
            }
            WasiType::Integer32 { .. } => {
                todo!()
            }
            WasiType::Integer64 { .. } => {
                todo!()
            }
            WasiType::Float32 => {
                todo!()
            }
            WasiType::Float64 => {
                todo!()
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
