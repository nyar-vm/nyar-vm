use convert_case::Casing;

use super::*;

#[derive(Clone, Debug)]
pub struct Identifier {
    pub namepath: Vec<Arc<str>>,
    pub name: Arc<str>,
}

impl Identifier {
    pub(crate) fn wasi_name(&self) -> String {
        self.name.as_ref().to_case(Case::Kebab)
    }
}

impl From<&str> for Identifier {
    fn from(value: &str) -> Self {
        Self { namepath: vec![], name: Arc::from(value) }
    }
}

impl From<Arc<str>> for Identifier {
    fn from(value: Arc<str>) -> Self {
        Self { namepath: vec![], name: value }
    }
}

impl Identifier {
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { namepath: Vec::new(), name: name.into() }
    }
}

impl FromStr for Identifier {
    type Err = SyntaxError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split("::");
        let mut namepath = Vec::new();
        let name = parts.next().ok_or(SyntaxError::new("No name found"))?;
        for part in parts {
            namepath.push(Arc::from(part));
        }
        Ok(Self { namepath, name: Arc::from(name) })
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for path in &self.namepath {
            write!(f, "{}::", path)?;
        }
        write!(f, "{}", self.name)
    }
}
