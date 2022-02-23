use crate::ValkyrieIdentifier;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassDeclare {
    pub name: String,
    pub modifiers: Vec<ValkyrieIdentifier>,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub statements: Vec<ValkyrieASTNode>,
}

pub struct ClassItemDeclare {
    pub name: String,
    pub modifiers: Vec<ValkyrieIdentifier>,
    pub ty: ValkyrieASTNode,
    pub value: Option<ValkyrieASTNode>,
}

impl ClassDeclare {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), modifiers: Vec::new(), extends: None, implements: Vec::new(), statements: Vec::new() }
    }
    pub fn mut_modifiers(&mut self) -> &mut Vec<ValkyrieIdentifier> {
        &mut self.modifiers
    }
    pub fn mut_statement(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.statements
    }
}
