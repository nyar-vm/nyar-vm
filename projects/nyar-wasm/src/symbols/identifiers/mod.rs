use super::*;

mod convert;
mod display;

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Identifier {
    /// The namespace of the identifier, only valid when name is not empty
    pub namespace: Vec<Arc<str>>,
    /// The name of the identifier, anonymous identifier is empty
    pub name: Arc<str>,
}

impl Default for Identifier {
    fn default() -> Self {
        Self { namespace: Vec::new(), name: Arc::from("") }
    }
}

impl Identifier {
    /// Check if it is an anonymous identifier
    pub fn is_anonymous(&self) -> bool {
        self.name.is_empty()
    }
    pub(crate) fn wasi_name(&self) -> String {
        self.name.as_ref().to_case(Case::Kebab)
    }
    pub(crate) fn wasi_id(&self) -> String {
        encode_id(&format!("{self:#}"))
    }
}

impl Identifier {
    /// Create a new identifier without namespace
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { namespace: Vec::new(), name: name.into() }
    }
    /// Create a new identifier with current [Identifier] as namespace
    pub fn join<S>(&self, name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        match self.name.as_ref() {
            "" => Self { namespace: self.namespace.clone(), name: name.into() },
            _ => {
                let mut namespace = self.namespace.clone();
                namespace.push(self.name.clone());
                Self { namespace, name: name.into() }
            }
        }
    }
}
