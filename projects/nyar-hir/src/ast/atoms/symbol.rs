use super::*;

///
#[derive(Debug, Clone)]
pub struct Symbol {
    name: String,
    scope: Vec<String>,
}

impl Symbol {
    pub fn simple(name: &str) -> Symbol {
        Self {
            name: String::from(name),
            scope: vec![]
        }
    }
    pub fn namespace(name: &str) -> Symbol {
        Self {
            name: String::from(name),
            scope: vec![]
        }
    }
    pub fn path(names: &[String]) -> Symbol {
        let mut path = Vec::from(names);
        let name  = path.pop().unwrap();
        Self {
            name,
            scope: path
        }
    }
}