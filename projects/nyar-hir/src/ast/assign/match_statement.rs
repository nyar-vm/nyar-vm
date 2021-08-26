pub struct MatchStatement {}

pub struct CatchStatement {}

pub struct BuilderStatement {
    head: String,
}

impl Default for BuilderStatement {
    fn default() -> Self {
        Self { head: "".to_string() }
    }
}

impl BuilderStatement {
    pub fn push_head(&mut self, head: &str) {
        self.head = head.to_string()
    }
}
