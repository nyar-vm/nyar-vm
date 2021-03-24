use super::*;

///
/// ```v
/// ++base!!
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnaryArgument {
    pub prefix: Vec<Operator>,
    pub suffix: Vec<Operator>,
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
