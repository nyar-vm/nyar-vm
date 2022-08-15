use super::*;

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
#[derive(Clone, Debug)]
pub struct EnumerationTable {
    pub branches: BTreeMap<u32, Vec<Operation>>,
    pub default: Vec<Operation>,
    pub r#return: Vec<WasmType>,
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
#[derive(Clone, Debug)]
pub struct JumpTable {
    pub branches: Vec<JumpCondition>,
    pub default: Vec<Operation>,
    pub r#return: Vec<WasmType>,
}

#[derive(Clone, Debug)]
pub struct JumpBranch {
    /// The conditional jump branch
    pub main: JumpCondition,
    /// The default jump branch
    pub default: Vec<Operation>,
    /// Compile with `select`, otherwise `if else end` will be used
    pub select: bool,
    /// The return type of the block
    pub r#return: Vec<WasmType>,
}

#[derive(Clone, Debug)]
pub struct JumpCondition {
    pub condition: Vec<Operation>,
    pub action: Vec<Operation>,
}

impl JumpBranch {
    /// Create a `if ... then ... end` branch
    pub fn if_then(r#if: Vec<Operation>, then: Vec<Operation>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: vec![], select: false, r#return: vec![] }
    }
    /// Create a `if ... then ... else ... end` branch
    pub fn if_then_else(r#if: Vec<Operation>, then: Vec<Operation>, r#else: Vec<Operation>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: r#else, select: false, r#return: vec![] }
    }
    /// Set the return type of the if statement
    pub fn with_return_type(self, r#type: Vec<WasmType>) -> Self {
        Self { r#return: r#type, ..self }
    }
    /// Whether to execute branches in parallel
    pub fn with_parallel(self, select: bool) -> Self {
        Self { select, ..self }
    }
}

impl JumpCondition {
    pub fn new(condition: Vec<Operation>, action: Vec<Operation>) -> Self {
        Self { condition, action }
    }
}

impl Operation {
    pub fn if_then(r#if: Vec<Operation>, then: Vec<Operation>) -> Self {
        JumpBranch::if_then(r#if, then).into()
    }
    pub fn if_then_else(r#if: Vec<Operation>, then: Vec<Operation>, r#else: Vec<Operation>) -> Self {
        JumpBranch::if_then_else(r#if, then, r#else).into()
    }
}
