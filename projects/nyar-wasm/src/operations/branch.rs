use super::*;

#[derive(Clone, Debug)]
pub struct JumpTable {
    pub branches: Vec<JumpCondition>,
    pub default: Vec<Operation>,
    pub r#return: Vec<WasmType>,
}

#[derive(Clone, Debug)]
pub struct JumpBranch {
    pub main: JumpCondition,
    pub default: Vec<Operation>,
    pub r#return: Vec<WasmType>,
}

#[derive(Clone, Debug)]
pub struct JumpCondition {
    pub condition: Vec<Operation>,
    pub action: Vec<Operation>,
}

impl From<JumpBranch> for Operation {
    fn from(value: JumpBranch) -> Self {
        Self::JumpBranch(value)
    }
}

impl JumpBranch {
    pub fn if_then(r#if: Vec<Operation>, then: Vec<Operation>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: vec![], r#return: vec![] }
    }
    pub fn if_then_else(r#if: Vec<Operation>, then: Vec<Operation>, r#else: Vec<Operation>) -> Self {
        Self { main: JumpCondition { condition: r#if, action: then }, default: r#else, r#return: vec![] }
    }
    pub fn with_return_type(self, r#type: Vec<WasmType>) -> Self {
        Self { r#return: r#type, ..self }
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
