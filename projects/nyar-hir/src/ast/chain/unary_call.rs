use super::*;

///
/// ```v
/// ++base!!
/// ```
#[derive(Clone, Debug)]
pub struct UnaryCall {
    pub base: ASTNode,
    pub prefix: Vec<Operator>,
    pub suffix: Vec<Operator>,
}

impl UnaryCall {
    pub fn new(base: ASTNode) -> Self {
        Self { base, prefix: vec![], suffix: vec![] }
    }
    pub fn push_prefix(&mut self, ops: &[String]) {
        for op in ops {
            self.prefix.push(Operator::parse(op, -1))
        }
    }
    pub fn push_suffix(&mut self, ops: &[String]) {
        for op in ops {
            self.suffix.push(Operator::parse(op, 1))
        }
    }
}
