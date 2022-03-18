use nyar_error::FileSpan;
use std::{
    fmt::{Display, Formatter},
    rc::Rc,
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Identifier {
    path: Vec<Symbol>,
    span: FileSpan,
}
#[derive(Clone, Debug)]
pub struct Symbol {
    inner: Arc<str>,
}

impl Symbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, symbol) in self.path.iter().enumerate() {
            if i != 0 {
                f.write_str("::")?;
            }
            f.write_str(&symbol.inner)?;
        }
        Ok(())
    }
}

impl FromIterator<Symbol> for Identifier {
    fn from_iter<T: IntoIterator<Item = Symbol>>(iter: T) -> Self {
        Self { path: iter.into_iter().collect(), span: Default::default() }
    }
}
