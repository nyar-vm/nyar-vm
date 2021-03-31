use super::*;

/// A Symbol
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub scope: Vec<String>,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for s in &self.scope {
            write_identifier(s, f)?;
            f.write_str("::")?;
        }
        write_identifier(&self.name, f)
    }
}

fn write_identifier(id: &str, f: &mut Formatter) -> std::fmt::Result {
    match is_valid_identifier(id) {
        true => write!(f, "{}", id),
        false => write!(f, "`{}`", id),
    }
}

fn is_valid_identifier(id: &str) -> bool {
    id.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl Symbol {
    pub fn atom<S>(name: S) -> Symbol
    where
        S: Into<String>,
    {
        Self { name: name.into(), scope: vec![] }
    }
    pub fn path(names: Vec<&str>) -> Symbol {
        let mut path: Vec<_> = names.iter().map(|f| f.to_string()).collect();
        let name = path.pop().unwrap();
        Self { name, scope: path }
    }
    pub fn join(names: Vec<Symbol>) -> Symbol {
        for name in &names {
            debug_assert!(name.scope.is_empty())
        }
        let mut path: Vec<_> = names.into_iter().map(|f| f.name).collect();
        let name = path.pop().unwrap();
        Self { name, scope: path }
    }
}

#[test]
fn test_display() {
    assert_eq!(Symbol::path(vec!["a", "b", "c"]).to_string(), "a::b::c");
    assert_eq!(Symbol::path(vec!["a", "b", "℃"]).to_string(), "a::b::`℃`");
}
