use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "'de: 'static"))]
pub struct LoopStatement {
    pub body: Vec<ASTNode>,
}

#[derive(Clone, Debug)]
pub struct WhileLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    else_trigger: Option<Vec<ASTNode>>,
}

#[derive(Clone, Debug)]
pub struct ForInLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    else_trigger: Option<Vec<ASTNode>>,
}

impl WhileLoop {
    pub fn new(condition: ASTNode, body: Vec<ASTNode>, meta: ASTMeta) -> ASTNode {
        Self { condition, body, else_trigger: None }.de_sugar(meta)
    }
    pub fn while_else(condition: ASTNode, body: Vec<ASTNode>, else_trigger: Vec<ASTNode>, meta: ASTMeta) -> ASTNode {
        Self { condition, body, else_trigger: Some(else_trigger) }.de_sugar(meta)
    }
    fn de_sugar(&self, meta: ASTMeta) -> ASTNode {
        if self.condition.is_true() {
            return ASTNode::loop_statement(self.body.clone(), meta);
        }
        match &self.else_trigger {
            None => ASTNode::loop_statement(loop_body, meta),
            Some(s) => ASTNode::if_statement(IfStatement::if_else(condition, loop_body, s), meta),
        }
    }
    // loop {
    //     if cond {
    //         body1
    //     }
    //     else {
    //         break
    //     }
    // }
    fn de_sugar_loop(&self, meta: ASTMeta) {
        let break_body = vec![ASTNode::control_break(self.condition.span.clone())];
        let cond = IfStatement::if_else(self.condition.clone(), self.body.clone(), break_body);
        let if_body = ASTNode::if_statement(cond, meta.clone());
        let loop_body = ASTNode::loop_statement(vec![cond], self.meta.clone());
    }
}

impl ForInLoop {
    pub fn de_sugar(self) -> LoopStatement {
        todo!()
    }
}
