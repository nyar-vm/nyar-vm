use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContinuationArgument {
    pub body: Vec<ASTNode>,
}

impl ChainCall {
    pub fn push_continuation(&mut self, rhs: Vec<ASTNode>) {
        self.chain.push(CallableItem::BlockCall(ContinuationArgument { body: rhs }));
    }
}
