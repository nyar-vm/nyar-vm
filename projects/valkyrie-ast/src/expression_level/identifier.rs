use super::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ValkyrieIdentifier {
    pub name: String,
    pub span: FileSpan,
}

impl Display for ValkyrieIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // if string is pure xid
        write!(f, "{}", self.name)
        // if self.name.chars().all(|c| c.is_xid_continue()) {
        //     write!(f, "{}", self.name)
        // } else {
        //     write!(f, "\"{}\"", self.name)
        // }
    }
}

impl ValkyrieIdentifier {
    pub fn new(name: impl Into<String>, file: FileID, range: &Range<usize>) -> Self {
        Self { name: name.into(), span: FileSpan { file, head: range.start, tail: range.end } }
    }
}

impl ValkyrieASTNode {
    pub fn identifier(name: impl Into<String>, file: FileID, range: &Range<usize>) -> ValkyrieIdentifier {
        ValkyrieIdentifier::new(name, file, range)
    }
    pub fn namepath(items: Vec<ValkyrieIdentifier>, file: FileID, range: &Range<usize>) -> Self {
        Self { kind: ValkyrieASTKind::Namepath(items), span: FileSpan::new(file, range) }
    }
}
