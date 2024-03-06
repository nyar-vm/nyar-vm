use super::*;

mod convert;
mod display;

#[derive(Clone, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub struct Identifier {
    pub namespace: Vec<Arc<str>>,
    pub name: Arc<str>,
}

impl Default for Identifier {
    fn default() -> Self {
        Self { namespace: Vec::new(), name: Arc::from("") }
    }
}

impl Identifier {
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
    pub fn new<S>(name: S) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { namespace: Vec::new(), name: name.into() }
    }
}
