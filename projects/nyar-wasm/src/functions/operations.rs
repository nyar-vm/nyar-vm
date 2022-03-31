use super::*;

impl Operation {
    pub fn r#loop(label: &str, body: Vec<Operation>) -> Self {
        Self::Loop {
            r#continue: Symbol::new(&format!("{label}@continue")),
            r#break: Symbol::new(&format!("{label}@break")),
            body,
        }
    }
    pub fn r#continue(label: &str) -> Self {
        Self::Goto { label: Symbol::new(&format!("{label}@continue")) }
    }
    pub fn r#break(label: &str) -> Self {
        Self::Goto { label: Symbol::new(&format!("{label}@break")) }
    }
}
