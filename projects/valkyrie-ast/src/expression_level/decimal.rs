use valkyrie_errors::ValkyrieError;

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieDecimalNode {
    pub hint: ValkyrieIdentifier,
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

impl ValkyrieASTNode {
    pub fn decimal(num: &str, file: FileID, range: &Range<usize>, hint: Option<ValkyrieIdentifier>) -> ValkyrieResult<Self> {
        match FBig::from_str(num) {
            Ok(o) => Ok(ValkyrieDecimalNode { hint: hint.unwrap_or_default(), value: o }.to_node(file, range)),
            Err(_) => Err(ValkyrieError::syntax_error(format!("Invalid decimal number: {}", num), FileSpan::new(file, range))),
        }
    }
}
