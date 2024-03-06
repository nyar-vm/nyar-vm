use crate::{
    encoder::WastEncoder,
    helpers::{EmitConstant, EmitDefault, ToWasiType},
    operations::branch::EnumerationTable,
    Identifier, JumpBranch, JumpTable, WasiType, WasiValue,
};
use std::fmt::Write;

pub mod branch;

pub(crate) trait Emit {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

pub(crate) trait EmitValue {
    fn emit_default<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
    fn emit_constant<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasiInstruction {
    /// Create the default value for a given type
    Default(WasiType),
    /// Create a constant value
    Constant(WasiValue),
    /// Convert stack value to WASI type
    Convert {
        /// The target type after convert
        into: WasiType,
    },
    /// Transmute stack value to WASI type
    Transmute {
        /// The target type after transmute
        into: WasiType,
    },
    GetField,
    SetField,
    CallFunction {
        symbol: Identifier,
    },
    NativeSum {
        terms: Vec<WasiInstruction>,
    },
    NativeProduct {
        terms: Vec<WasiInstruction>,
    },
    /// `if cond { } else { }`
    JumpBranch(JumpBranch),
    /// `if c1 { } else if c2 { } else { }`
    JumpTable(JumpTable),
    /// `case 0: ... else: ...`
    JumpEnumeration(EnumerationTable),
    Goto {},
    Return {},
    Drop {
        objects: usize,
    },
}

impl WasiInstruction {
    /// Create a const type
    pub fn constant<T>(value: T) -> Self
    where
        T: Into<WasiValue>,
    {
        Self::Constant(value.into())
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn emit_instructions(&mut self, instruction: &[WasiInstruction]) -> std::fmt::Result {
        for i in instruction {
            i.emit(self)?;
        }
        Ok(())
    }
}

impl Emit for WasiInstruction {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            WasiInstruction::Default(v) => {
                v.emit_default(w)?;
                w.stack.push(v.clone())
            }
            WasiInstruction::Constant(v) => {
                v.emit_constant(w)?;
                w.stack.push(v.to_wasi_type())
            }
            WasiInstruction::Convert { into } => {
                let last = w.stack.pop();
                match last {
                    Some(last) => {
                        last.emit_convert(into, w)?;
                        w.stack.push(into.clone())
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            WasiInstruction::Transmute { into } => {
                let last = w.stack.pop();
                match last {
                    Some(last) => {
                        last.emit_transmute(into, w)?;
                        w.stack.push(into.clone())
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            WasiInstruction::GetField => {
                todo!()
            }
            WasiInstruction::SetField => {
                todo!()
            }
            WasiInstruction::CallFunction { symbol } => match w.source.get_function(symbol) {
                Some(s) => {
                    write!(w, "call {}", symbol.wasi_id())?;
                    for input in s.inputs.iter() {
                        match w.stack.pop() {
                            Some(s) => {
                                if s.ne(&input.r#type) {
                                    panic!("Mismatch type")
                                }
                            }
                            None => {
                                panic!("Missing parameter")
                            }
                        }
                    }
                    for output in s.output.clone() {
                        w.stack.push(output)
                    }
                }
                None => {
                    panic!("Missing function")
                }
            },
            // (drop drop ...)
            WasiInstruction::Drop { objects } => write!(w, "({})", "drop ".repeat(*objects).trim_end())?,
            WasiInstruction::Goto { .. } => {
                todo!()
            }
            WasiInstruction::Return { .. } => {
                todo!()
            }
            WasiInstruction::NativeSum { .. } => {
                todo!()
            }
            WasiInstruction::NativeProduct { .. } => {
                todo!()
            }
            WasiInstruction::JumpBranch(v) => v.emit(w)?,
            WasiInstruction::JumpTable(_) => {
                todo!()
            }
            WasiInstruction::JumpEnumeration(_) => {
                todo!()
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
