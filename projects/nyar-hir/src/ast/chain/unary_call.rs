use super::*;

///
/// ```v
/// ++base!!
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct UnaryArgument {
    pub prefix: Vec<Operator>,
    pub suffix: Vec<Operator>,
}

impl Debug for UnaryArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let prefix: Vec<_> = self.prefix.iter().map(|f| f.to_string()).collect();
        let suffix: Vec<_> = self.suffix.iter().map(|f| f.to_string()).collect();
        f.debug_struct("UnaryArgument")
            .field("prefix", &format!("[{}]", prefix.join(", ")))
            .field("suffix", &format!("[{}]", suffix.join(", ")))
            .finish()
    }
}

impl Default for UnaryArgument {
    fn default() -> Self {
        Self { prefix: vec![], suffix: vec![] }
    }
}

impl AddAssign<UnaryArgument> for ChainCall {
    fn add_assign(&mut self, rhs: UnaryArgument) {
        if rhs.is_empty() {
            return;
        }
        self.chain.push(CallableItem::UnaryCall(rhs));
    }
}

impl UnaryArgument {
    pub fn push_prefix(&mut self, o: &str) {
        self.prefix.push(Operator::parse_prefix(o))
    }
    pub fn push_suffix(&mut self, o: &str) {
        self.suffix.push(Operator::parse_postfix(o))
    }
    pub fn is_empty(&self) -> bool {
        self.prefix.is_empty() && self.suffix.is_empty()
    }
}
