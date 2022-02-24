use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassDeclare {
    namepath: Vec<ValkyrieIdentifier>,
    modifiers: Vec<ValkyrieIdentifier>,
    extends: Option<String>,
    implements: Vec<String>,
    statements: Vec<ValkyrieASTNode>,
}

impl Default for ClassDeclare {
    fn default() -> Self {
        Self { namepath: vec![], modifiers: vec![], extends: None, implements: vec![], statements: vec![] }
    }
}

impl ClassDeclare {
    pub fn get_namepath(&self) -> Iter<'_, ValkyrieIdentifier> {
        self.namepath.iter()
    }
    pub fn mut_namepath(&mut self) -> &mut Vec<ValkyrieIdentifier> {
        &mut self.namepath
    }
    pub fn get_modifiers(&self) -> Iter<'_, ValkyrieIdentifier> {
        self.modifiers.iter()
    }
    pub fn mut_modifiers(&mut self) -> &mut Vec<ValkyrieIdentifier> {
        &mut self.modifiers
    }
    pub fn get_statement(&self) -> Iter<'_, ValkyrieASTNode> {
        self.statements.iter()
    }
    pub fn mut_statement(&mut self) -> &mut Vec<ValkyrieASTNode> {
        &mut self.statements
    }
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTKind::Class(box self).to_node(file, range)
    }
}
