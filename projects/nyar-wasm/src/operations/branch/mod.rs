use super::*;
use std::collections::BTreeMap;

/// ```v
/// match var {
///     case 0:
///         ...
///     case 2:
///         ...
///     else:
///         ...
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EnumerationTable {
    pub branches: BTreeMap<u32, Vec<WasiInstruction>>,
    pub default: Vec<WasiInstruction>,
    pub r#return: Vec<WasiType>,
}

/// ```v
/// switch {
///     when x == 0:
///         ...
///     when x == 2:
///         ...
///     else:
///         ...
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JumpTable {
    pub branches: Vec<JumpCondition>,
    pub default: Vec<WasiInstruction>,
    pub r#return: Vec<WasiType>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JumpBranch {
    /// The conditional jump branch
    pub main: JumpCondition,
    /// The default jump branch
    pub default: Vec<WasiInstruction>,
    /// Compile with `select`, otherwise `if else end` will be used
    pub select: bool,
    /// The return type of the block
    pub r#return: Vec<WasiType>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JumpCondition {
    /// The condition of the if statement
    pub condition: Vec<WasiInstruction>,
    /// The action if condition is true
    pub action: Vec<WasiInstruction>,
}

impl JumpBranch {
    /// Create a `if ... then ... end` branch
    pub fn if_then(r#if: Vec<WasiInstruction>, then: Vec<WasiInstruction>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: vec![], select: false, r#return: vec![] }
    }
    /// Create a `if ... then ... else ... end` branch
    pub fn if_then_else(r#if: Vec<WasiInstruction>, then: Vec<WasiInstruction>, r#else: Vec<WasiInstruction>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: r#else, select: false, r#return: vec![] }
    }
    /// Set the return type of the if statement
    pub fn with_return_type(self, r#type: Vec<WasiType>) -> Self {
        Self { r#return: r#type, ..self }
    }
    /// Whether to execute branches in parallel
    pub fn with_parallel(self, select: bool) -> Self {
        Self { select, ..self }
    }
}

impl JumpCondition {
    pub fn new(condition: Vec<WasiInstruction>, action: Vec<WasiInstruction>) -> Self {
        Self { condition, action }
    }
}

impl WasiInstruction {
    pub fn if_then(r#if: Vec<WasiInstruction>, then: Vec<WasiInstruction>) -> Self {
        JumpBranch::if_then(r#if, then).into()
    }
    pub fn if_then_else(r#if: Vec<WasiInstruction>, then: Vec<WasiInstruction>, r#else: Vec<WasiInstruction>) -> Self {
        JumpBranch::if_then_else(r#if, then, r#else).into()
    }
}

impl Emit for JumpBranch {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        for i in &self.main.condition {
            i.emit(w)?;
        }
        let recover = std::mem::take(&mut w.stack);
        write!(w, "(if")?;
        w.indent();
        write!(w, "(then")?;
        w.indent();
        for i in &self.main.action {
            i.emit(w)?;
        }
        w.dedent(1);
        write!(w, "(else")?;
        w.indent();
        for i in &self.default {
            i.emit(w)?;
        }
        w.dedent(1);
        w.dedent(1);
        w.stack = recover;
        Ok(())
    }
}

impl From<JumpBranch> for WasiInstruction {
    fn from(value: JumpBranch) -> Self {
        Self::JumpBranch(value)
    }
}
