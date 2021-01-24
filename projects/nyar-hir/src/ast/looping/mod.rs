use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]

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
    pub fn new(condition: ASTNode, body: Vec<ASTNode>, span: Span) -> ASTNode {
        Self { condition, body, else_trigger: None }.de_sugar(span)
    }
    pub fn while_else(condition: ASTNode, body: Vec<ASTNode>, else_trigger: Vec<ASTNode>, span: Span) -> ASTNode {
        Self { condition, body, else_trigger: Some(else_trigger) }.de_sugar(span)
    }
    fn de_sugar(&self, span: Span) -> ASTNode {
        if self.condition.is_true() {
            return ASTNode::loop_statement(self.body.clone(), span);
        }
        let loop_body = self.de_sugar_loop(span);
        match &self.else_trigger {
            None => loop_body,
            Some(s) => {
                let if_body = IfStatement::if_else(self.condition.clone(), vec![loop_body], s.clone());
                ASTNode::if_statement(if_body, span)
            }
        }
    }
    fn de_sugar_loop(&self, meta: Span) -> ASTNode {
        let break_body = vec![ASTNode::control_break(self.condition.span.clone())];
        let cond = IfStatement::if_else(self.condition.clone(), self.body.clone(), break_body);
        let if_body = ASTNode::if_statement(cond, meta.clone());
        ASTNode::loop_statement(vec![if_body], meta.clone())
    }
}

impl ForInLoop {
    pub fn new(condition: ASTNode, body: Vec<ASTNode>, span: Span) -> ASTNode {
        Self { condition, body, else_trigger: None }.de_sugar(span)
    }
    pub fn while_else(condition: ASTNode, body: Vec<ASTNode>, else_trigger: Vec<ASTNode>, span: Span) -> ASTNode {
        Self { condition, body, else_trigger: Some(else_trigger) }.de_sugar(span)
    }
    fn de_sugar(&self, span: Span) -> ASTNode {
        if self.condition.is_true() {
            return ASTNode::loop_statement(self.body.clone(), span);
        }
        let loop_body = self.de_sugar_loop(span);
        match &self.else_trigger {
            None => loop_body,
            Some(s) => {
                let if_body = IfStatement::if_else(self.condition.clone(), vec![loop_body], s.clone());
                ASTNode::if_statement(if_body, span)
            }
        }
    }
    fn de_sugar_loop(&self, meta: Span) -> ASTNode {
        let break_body = vec![ASTNode::control_break(self.condition.span.clone())];
        let cond = IfStatement::if_else(self.condition.clone(), self.body.clone(), break_body);
        let if_body = ASTNode::if_statement(cond, meta.clone());
        ASTNode::loop_statement(vec![if_body], meta.clone())
    }
}
