use super::*;

pub struct ClassFieldDeclare {
    pub name: Vec<ValkyrieIdentifier>,
    pub modifiers: Vec<ValkyrieIdentifier>,
    pub ty: ValkyrieASTNode,
    pub value: Option<ValkyrieASTNode>,
}
