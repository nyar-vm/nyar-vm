use crate::{
    helpers::{Id, WasmInstruction, WasmOutput},
    types::WasmType,
    values::WasmValue,
    EnumerationTable, JumpBranch, JumpTable, WasmSymbol,
};
use std::collections::BTreeMap;
use wast::{
    core::{BlockType, Instruction, TableArg, TypeUse},
    token::{Float32, Float64, Index},
};

mod codegen;
mod convert;

pub mod branch;

#[derive(Clone, Debug)]
pub enum Operation {
    Sequence {
        code: Vec<Operation>,
    },
    Repeats {
        code: Vec<Operation>,
        repeats: usize,
    },
    CallFunction {
        name: WasmSymbol,
        input: Vec<Operation>,
    },
    GetVariable {
        kind: VariableKind,
        variable: WasmSymbol,
    },
    SetVariable {
        kind: VariableKind,
        variable: WasmSymbol,
    },
    TeeVariable {
        variable: WasmSymbol,
    },
    Loop {
        r#continue: WasmSymbol,
        r#break: WasmSymbol,
        body: Vec<Operation>,
    },
    Goto {
        label: WasmSymbol,
    },
    Drop,
    Return,
    Unreachable,

    /// `if cond { } else { }`
    JumpBranch(JumpBranch),
    /// `if c1 { } else if c2 { } else { }`
    JumpTable(JumpTable),
    /// `case 0: ... else: ...`
    JumpEnumeration(EnumerationTable),
    Default {
        typed: WasmType,
    },
    Constant {
        value: WasmValue,
    },
    NativeSum {
        native: WasmType,
        terms: Vec<Operation>,
    },
    Convert {
        from: WasmType,
        into: WasmType,
        code: Vec<Operation>,
    },
    Transmute {
        from: WasmType,
        into: WasmType,
        code: Vec<Operation>,
    },
    NativeEqual {
        native: WasmType,
        codes: Vec<Operation>,
    },
    NativeEqualZero {
        native: WasmType,
        term: Box<Operation>,
    },
}

#[derive(Copy, Clone, Debug)]
pub enum VariableKind {
    Global,
    Local,
    Table,
}

impl Operation {
    pub fn r#loop(label: &str, body: Vec<Operation>) -> Self {
        Self::Loop {
            r#continue: WasmSymbol::new(&format!("{label}@continue")),
            r#break: WasmSymbol::new(&format!("{label}@break")),
            body,
        }
    }
    pub fn r#continue(label: &str) -> Self {
        Self::Goto { label: WasmSymbol::new(&format!("{label}@continue")) }
    }
    pub fn r#break(label: &str) -> Self {
        Self::Goto { label: WasmSymbol::new(&format!("{label}@break")) }
    }
    pub fn local_get<S: Into<WasmSymbol>>(name: S) -> Self {
        Self::GetVariable { kind: VariableKind::Local, variable: name.into() }
    }
    pub fn local_set<S: Into<WasmSymbol>>(name: S) -> Self {
        Self::SetVariable { kind: VariableKind::Local, variable: name.into() }
    }
    pub fn local_tee<S: Into<WasmSymbol>>(name: S) -> Self {
        Self::TeeVariable { variable: name.into() }
    }
    pub fn drop(count: usize) -> Self {
        Self::Repeats { code: vec![Self::Drop], repeats: count }
    }
}
