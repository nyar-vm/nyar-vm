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
    ///
    Conversion {
        from: WasiType,
        into: WasiType,
    },
    GetField,
    SetField,
    CallFunction {
        symbol: Identifier,
        parameters: Vec<WasiInstruction>,
    },
    NativeSum {
        terms: Vec<WasiInstruction>,
    },
    NativeProduct {
        terms: Vec<WasiInstruction>,
    },
    Goto {},
    Return {},
    Drop {
        objects: usize,
    },
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn emit_instructions(&mut self, instruction: &[WasiInstruction]) -> std::fmt::Result {
        for i in instruction {
            match i {
                WasiInstruction::Default(v) => v.emit_default(self)?,
                WasiInstruction::Constant(v) => v.emit_constant(self)?,
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
                    write!(self, "(call {}", symbol.wasi_id())?;
                    self.newline()?;
                    self.indent();
                    self.emit_instructions(parameters)?;
                    self.dedent(1)
                }
                // (drop drop ...)
                WasiInstruction::Drop { objects } => {
                    write!(self, "({})", "drop".repeat(*objects).join(" "))
                }
                WasiInstruction::Goto { .. } => {
                    todo!()
                }
                WasiInstruction::Return { .. } => {
                    todo!()
                }
                WasiInstruction::NativeSum { .. } => {}
                WasiInstruction::NativeProduct { .. } => {}
            }
        }
        Ok(())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum VariableKind {
    Global,
    Local,
    Table,
}
