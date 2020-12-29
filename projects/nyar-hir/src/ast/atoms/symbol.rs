use super::*;

///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Symbol {
    pub name: String,
    pub scope: Vec<String>,
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        for s in &self.scope {
            f.write_str(s)?;
            f.write_str("::")?;
        }
        f.write_str(&self.name)
    }
}

impl Symbol {
    pub fn simple<S>(name: S) -> Symbol
    where
        S: Into<String>,
    {
        Self { name: name.into(), scope: vec![] }
    }
    pub fn namespace(name: &str) -> Symbol {
        Self { name: String::from(name), scope: vec![] }
    }
    pub fn path(names: &[String]) -> Symbol {
        let mut path = Vec::from(names);
        let name = path.pop().unwrap();
        Self { name, scope: path }
    }
}
