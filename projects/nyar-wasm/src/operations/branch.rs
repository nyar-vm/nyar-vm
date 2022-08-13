use super::*;

#[derive(Debug)]
pub struct JumpBranch {
    pub condition: Vec<Operation>,
    pub then: Vec<Operation>,
    pub r#else: Vec<Operation>,
    pub r#return: Vec<WasmType>,
}

impl From<JumpBranch> for Operation {
    fn from(value: JumpBranch) -> Self {
        Self::JumpBranch(value)
    }
}

impl JumpBranch {
    pub fn if_then(condition: Vec<Operation>, then: Vec<Operation>) -> Self {
        Self { condition, then, r#else: vec![], r#return: vec![] }
    }
    pub fn is_then_else(condition: Vec<Operation>, then: Vec<Operation>, r#else: Vec<Operation>) -> Self {
        Self { condition, then, r#else, r#return: vec![] }
    }
    pub fn with_return_type(self, r#type: Vec<WasmType>) -> Self {
        Self { r#return: r#type, ..self }
    }
}

impl Operation {
    pub fn if_then(condition: Vec<Operation>, then: Vec<Operation>) -> Self {
        JumpBranch::if_then(condition, then).into()
    }
    pub fn if_then_else(condition: Vec<Operation>, then: Vec<Operation>, r#else: Vec<Operation>) -> Self {
        JumpBranch::is_then_else(condition, then, r#else).into()
    }
}
