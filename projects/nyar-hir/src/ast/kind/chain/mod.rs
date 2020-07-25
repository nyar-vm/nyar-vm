use super::*;

mod apply_call;
mod dict_call;
mod infix_call;
mod kont_call;
mod slice_call;
mod unary_call;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CallChain {
    pub base: ASTNode,
    pub chain: Vec<ASTNode>,
}

impl Default for CallChain {
    fn default() -> Self {
        Self { base: Default::default(), chain: vec![] }
    }
}

impl AddAssign<ASTNode> for CallChain {
    fn add_assign(&mut self, rhs: ASTNode) {
        self.chain.push(rhs)
    }
}

impl CallChain {
    pub fn new(ast: ASTNode) -> Self {
        Self { base: ast, chain: vec![] }
    }
}
