use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopStatement {
    pub body: Vec<ASTNode>,
}

#[derive(Clone, Debug)]
pub struct WhileLoop {
    condition: Option<ASTNode>,
    body: Vec<ASTNode>,
    else_trigger: Option<Vec<ASTNode>>,
}

#[derive(Clone, Debug)]
pub struct ForInLoop {
    condition: ASTNode,
    body: Vec<ASTNode>,
    else_trigger: Option<Vec<ASTNode>>,
}

impl Default for ForInLoop {
    fn default() -> Self {
        Self { condition: Default::default(), body: vec![], else_trigger: None }
    }
}

impl Default for WhileLoop {
    fn default() -> Self {
        Self { condition: None, body: vec![], else_trigger: None }
    }
}

impl WhileLoop {
    pub fn push_condition(&mut self, condition: ASTNode) {
        self.condition = Some(condition);
    }
    pub fn push_body(&mut self, body: Vec<ASTNode>) {
        self.body = body
    }
    pub fn push_else(&mut self, body: Vec<ASTNode>) {
        self.else_trigger = Some(body);
    }
    pub fn get_condition(&self) -> ASTNode {
        match &self.condition {
            None => ASTNode::boolean(true, Span::default()),
            Some(s) => s.clone(),
        }
    }
    pub fn always_true(&self) -> bool {
        self.get_condition().is_true()
    }
    fn de_sugar(&self, span: Span) -> ASTNode {
        if self.always_true() {
            return ASTNode::loop_statement(self.body.clone(), span);
        }
        let loop_body = self.de_sugar_loop(span);
        match &self.else_trigger {
            None => loop_body,
            Some(s) => IfStatement::if_else(self.get_condition(), vec![loop_body], s.clone()).as_node(span),
        }
    }
    fn de_sugar_loop(&self, span: Span) -> ASTNode {
        let cond = self.condition.clone().unwrap_or(ASTNode::boolean(true, span));
        let break_body = vec![ASTNode::control_break(cond.span.clone())];
        let cond = IfStatement::if_else(cond.clone(), self.body.clone(), break_body);
        let if_body = cond.as_node(span.clone());
        ASTNode::loop_statement(vec![if_body], span.clone())
    }
    pub fn as_node(&self, span: Span) -> ASTNode {
        self.de_sugar(span)
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
            Some(s) => IfStatement::if_else(self.condition.clone(), vec![loop_body], s.clone()).as_node(span),
        }
    }
    fn de_sugar_loop(&self, meta: Span) -> ASTNode {
        let break_body = vec![ASTNode::control_break(self.condition.span.clone())];
        let cond = IfStatement::if_else(self.condition.clone(), self.body.clone(), break_body);
        let if_body = cond.as_node(meta.clone());
        ASTNode::loop_statement(vec![if_body], meta.clone())
    }
    //noinspection RsSelfConvention
    pub fn as_node(self, span: Span) -> ASTNode {
        self.de_sugar(span)
    }
}
