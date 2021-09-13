
use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieDecimalNode {
    pub hint: ValkyrieIdentifierNode,
    pub value: FBig,
}

impl Display for ValkyrieDecimalNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.hint.name)
    }
}


impl ValkyrieDecimalNode {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::Decimal(box self),
            span: FileSpan { file, head: range.start, tail: range.end },
        }
    }
}
