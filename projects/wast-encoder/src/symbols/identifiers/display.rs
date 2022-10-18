use super::*;

impl Debug for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identifier").field("path", &self.namespace.join("∷")).field("name", &self.name).finish()
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for path in &self.namespace {
            write!(f, "{}∷", path)?;
        }
        write!(f, "{}", self.name)
    }
}
