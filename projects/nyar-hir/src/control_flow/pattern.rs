use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValkyriePattern {
    Expression(ValkyrieASTNode),
    CasePattern,
}
