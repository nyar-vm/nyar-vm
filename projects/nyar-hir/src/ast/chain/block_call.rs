use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct ContinuationArgument {
    pub body: Vec<ASTNode>,
}

impl Debug for ContinuationArgument {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write_tuple("Continuation", &self.body, f)
    }
}

impl ChainCall {
    pub fn push_continuation(&mut self, rhs: Vec<ASTNode>) {
        self.chain.push(CallableItem::BlockCall(ContinuationArgument { body: rhs }));
    }
}
