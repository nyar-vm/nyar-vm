use super::*;

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (i, symbol) in self.path.iter().enumerate() {
            if i != 0 {
                f.write_str(".")?;
            }
            f.write_str(&symbol.inner)?;
        }
        Ok(())
    }
}
