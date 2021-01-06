use crate::utils::meta_range_from_block;
use super::*;

pub struct LoopingStatement {
    body: Vec<ASTNode>,
}


pub struct WhileLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    not_triggered: Option<Vec<ASTNode>>,
}

pub struct ForInLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    not_triggered: Option<Vec<ASTNode>>,
}

impl WhileLoop {
    pub fn new(condition: ASTNode, body: Vec<ASTNode>) -> LoopingStatement {
        if condition.is_true() {
            return LoopingStatement { body };
        }
        Self {
            condition,
            body,
            not_triggered: None,
        }.de_sugar()
    }


    pub fn de_sugar(self) -> LoopingStatement {
        match &self.not_triggered {
            None => {
                let stop = ASTNode::control_break(self.condition.meta.clone());
                let cond = IfStatement::if_else(self.condition, self.body, stop);
                LoopingStatement {
                    body: vec![ASTNode::if_statement(cond, meta_range_from_block(&self.body))],
                }
            }
            Some(s) => {
                self.de_sugar_else(s)
            }
        }
    }
}

impl ForInLoop {
    pub fn de_sugar(self) -> LoopingStatement {
        todo!()
    }
}