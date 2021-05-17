use super::*;

/// A Symbol
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub scope: Vec<String>,
}

impl Default for Symbol {
    fn default() -> Self {
        Self { name: "".to_string(), scope: vec![] }
    }
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
    /// ```vk
    /// a + b + c
    /// => a::b::c
    /// ```
    pub fn join(names: Vec<Symbol>) -> Symbol {
        for name in &names {
            debug_assert!(name.scope.is_empty())
        }
        let mut path: Vec<_> = names.into_iter().map(|f| f.name).collect();
        let name = path.pop().unwrap();
        Self { name, scope: path }
    }
    /// ```vk
    /// a::b ++ c::d
    /// => a::b::c::d
    /// ```
    pub fn concat(&self, other: Symbol) -> Symbol {
        let name = other.name;
        let scope = if self.name.is_empty() {
            self.scope.iter().cloned().chain(other.scope.into_iter()).collect()
        }
        else {
            self.scope.iter().cloned().chain([self.name.clone()].into_iter()).chain(other.scope.into_iter()).collect()
        };
        Self { name, scope }
    }
}
