use super::*;

#[derive(Clone, Serialize, Deserialize)]
pub struct LambdaFunction {}

impl Default for LambdaFunction {
    fn default() -> Self {
        Self {}
    }
}

impl LambdaFunction {
    pub fn call(&self, node: &ASTNode) -> ASTNode {
        todo!()
    }
}
