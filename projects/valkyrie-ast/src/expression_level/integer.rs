use std::str::FromStr;

use valkyrie_errors::{SyntaxError, ValkyrieResult};

use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValkyrieIntegerNode {
    pub hint: ValkyrieIdentifier,
    pub value: IBig,
}

impl Display for ValkyrieIntegerNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.value, self.hint.name)
    }
}

impl ValkyrieIntegerNode {
    pub fn to_node(self, file: FileID, range: &Range<usize>) -> ValkyrieASTNode {
        ValkyrieASTNode {
            kind: ValkyrieASTKind::Integer(box self),
            span: FileSpan { file, head: range.start, tail: range.end },
        }
    }
}

impl ValkyrieASTNode {
    pub fn integer(num: &str, file: FileID, range: &Range<usize>, hint: Option<ValkyrieIdentifier>) -> ValkyrieResult<Self> {
        match IBig::from_str(num) {
            Ok(o) => Ok(ValkyrieIntegerNode { hint: hint.unwrap_or_default(), value: o }.to_node(file, range)),
            Err(e) => Err(SyntaxError::from(e).with_file(file).with_range(range))?,
        }
    }
}
