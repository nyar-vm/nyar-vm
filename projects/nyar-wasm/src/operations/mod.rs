use crate::{
    encoder::WastEncoder,
    helpers::{EmitConstant, EmitDefault, ToWasiType},
    operations::{
        branch::EnumerationTable,
        looping::{LoopEach, LoopRepeat, LoopUntilBody, LoopWhileBody},
    },
    Identifier, JumpBranch, JumpTable, WasiType, WasiValue,
};
use std::{fmt::Write, sync::Arc};

pub mod branch;
pub mod looping;

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
    CallFunction {
        symbol: Identifier,
    },
    CallMethod {},
    GetField {
        name: Arc<str>,
    },
    SetField {
        name: Arc<str>,
    },
    GetOffset {
        offset: usize,
    },
    SetOffset {
        offset: usize,
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
    Continue {
        label: Arc<str>,
    },
    Break {
        label: Arc<str>,
    },
    Return {},
    Drop {
        objects: usize,
    },
    Loop(LoopRepeat),
    LoopEach(LoopEach),
    LoopWhile(LoopWhileBody),
    LoopUntil(LoopUntilBody),
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
            Self::Default(v) => {
                w.newline()?;
                v.emit_default(w)?;
                w.stack.push(v.clone())
            }
            Self::Constant(v) => {
                w.newline()?;
                v.emit_constant(w)?;
                w.stack.push(v.to_wasi_type())
            }
            Self::Convert { into } => {
                let last = w.stack.pop();
                match last {
                    Some(last) => {
                        w.newline()?;
                        last.emit_convert(into, w)?;
                        w.stack.push(into.clone())
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            Self::Transmute { into } => {
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
            Self::GetField { name } => {
                let last = w.stack.pop();
                match last {
                    // (struct.get $Type $field $data)
                    Some(WasiType::Record(r)) => {
                        write!(w, "struct.get {} ${}", r.symbol.wasi_id(), name)?;
                    }
                    Some(other) => {
                        panic!("Expected record, got {:?}", other)
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            Self::SetField { name } => {
                let last = w.stack.pop();
                match last {
                    // (struct.set $Type $field $data)
                    Some(WasiType::Record(r)) => {
                        write!(w, "(struct.set {} ${}", r.symbol.wasi_id(), name)?;
                    }
                    Some(other) => {
                        panic!("Expected record, got {:?}", other)
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            Self::CallMethod { .. } => {
                todo!()
            }
            Self::GetOffset { offset } => {
                let last = w.stack.pop();
                match last {
                    // (array.get $Type $array $index)
                    Some(WasiType::Array(r)) => {
                        write!(w, "array.get {} ${}", r.symbol.wasi_id(), offset)?;
                        w.stack.push(r.r#type)
                    }
                    Some(other) => {
                        panic!("Expected record, got {:?}", other)
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            Self::SetOffset { offset } => {
                let last = w.stack.pop();
                match last {
                    // (array.set $Type $array $index)
                    Some(WasiType::Array(r)) => {
                        write!(w, "array.set {} ${}", r.symbol.wasi_id(), offset)?;
                        w.stack.push(r.r#type)
                    }
                    Some(other) => {
                        panic!("Expected record, got {:?}", other)
                    }
                    None => {
                        panic!("no item on stack!")
                    }
                }
            }
            Self::CallFunction { symbol } => match w.source.get_function(symbol) {
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
                        w.stack.push(output.r#type)
                    }
                }
                None => {
                    panic!("Missing function")
                }
            },
            Self::Goto { .. } => {
                todo!()
            }
            Self::Continue { label } => write!(w, "br ${}%continue", label)?,
            Self::Break { label } => write!(w, "br ${}%break", label)?,
            Self::Return { .. } => {
                todo!()
            }
            Self::NativeSum { .. } => {
                todo!()
            }
            Self::NativeProduct { .. } => {
                todo!()
            }
            Self::JumpBranch(v) => v.emit(w)?,
            Self::JumpTable(_) => {
                todo!()
            }
            Self::JumpEnumeration(_) => {
                todo!()
            }
            // (drop drop ...)
            Self::Drop { objects } => write!(w, "({})", "drop ".repeat(*objects).trim_end())?,
            Self::Loop(body) => body.emit(w)?,
            Self::LoopEach(body) => body.emit(w)?,
            Self::LoopWhile(body) => body.emit(w)?,
            Self::LoopUntil(body) => body.emit(w)?,
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
