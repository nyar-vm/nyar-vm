use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieStringNode {
    pub hint: ValkyrieIdentifier,
    pub value: Vec<ValkyrieASTNode>,
}

impl Display for ValkyrieStringNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
        // write!(f, "{}{}", self.value, self.hint.name)
    }
}

impl ValkyrieStringNode {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::StringInterpolation(box self),
            span: FileSpan { file, head: range.start, tail: range.end },
        }
    }
}

impl ValkyrieASTNode {
    pub fn string(text: impl Into<String>, file: FileID, range: &Range<usize>) -> Self {
        ValkyrieASTNode { kind: ValkyrieASTKind::String(text.into()), span: FileSpan::new(file, range) }
    }
    pub fn string_interpolation(
        text: Vec<ValkyrieASTNode>,
        hint: ValkyrieIdentifier,
        file: FileID,
        range: &Range<usize>,
    ) -> Self {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::StringInterpolation(box ValkyrieStringNode { hint, value: text }),
            span: FileSpan::new(file, range),
        }
    }
}
