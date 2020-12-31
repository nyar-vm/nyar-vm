use super::*;

pub struct LoopingStatement {
    body: Vec<ASTNode>,
}


pub struct WhileLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    not_triggered: Option<Vec<ASTNode>>
}

pub struct ForInLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    not_triggered: Option<Vec<ASTNode>>
}

impl WhileLoop {
    pub fn de_sugar(self) -> LoopingStatement {
        match &self.not_triggered {
            None => {self.de_sugar_simple()}
            Some(s) => {self.de_sugar_else(s)}
        }
    }
}

impl ForInLoop {
    pub fn de_sugar(self) -> LoopingStatement {
        todo!()
    }
}