use super::*;

#[derive(Clone, Serialize, Deserialize)]

pub struct DefinedFunction {}

#[derive(Clone, Debug, Serialize, Deserialize)]

pub struct LambdaFunction {
    pub arguments: Vec<Symbol>,
    pub body: Vec<ASTNode>,
    pub continuations: Vec<LambdaFunction>,
}

impl LambdaFunction {
    // pub fn block(body: ASTNode) -> Self {
    //     Self { arguments: vec![], body }
    // }
    // pub fn simple(args: Vec<&str>, body: ASTNode) -> Self {
    //     Self { arguments: args.iter().map(|s| Symbol::simple(*s)).collect(), body }
    // }

    pub fn call(&self, node: &ASTNode) -> ASTNode {
        todo!()
    }
}

fn test() {}
