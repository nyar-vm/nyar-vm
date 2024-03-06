use super::*;

impl From<&str> for Identifier {
    fn from(value: &str) -> Self {
        Self::from_str(value).unwrap()
    }
}

impl From<Arc<str>> for Identifier {
    fn from(value: Arc<str>) -> Self {
        Self::from_str(&value).unwrap()
    }
}
impl FromStr for Identifier {
    type Err = SyntaxError;

    /// `package::module::name`
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let names: Vec<Arc<str>> = s.split("::").map(Arc::from).collect();
        match names.as_slice() {
            [] => Err(SyntaxError::new("empty identifier")),
            [name] => Ok(Identifier { namespace: vec![], name: name.clone() }),
            [path @ .., name] => Ok(Identifier { namespace: path.to_vec(), name: name.clone() }),
        }
    }
}
