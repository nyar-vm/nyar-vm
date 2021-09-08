use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValkyrieIdentifierNode {
    pub name: String,
    pub span: FileSpan,
}

impl ValkyrieIdentifierNode {
    pub fn new(name: impl Into<String>, file: FileID, range: &Range<usize>) -> Self {
        Self { name: name.into(), span: FileSpan { file, head: range.start, tail: range.end } }
    }
    pub fn to_node(self) -> ValkyrieASTNode {
        let span = self.span.clone();
        ValkyrieASTNode { kind: ValkyrieASTKind::Identifier(box self), span }
    }
}
