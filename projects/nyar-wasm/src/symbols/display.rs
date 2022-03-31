use super::*;

impl Display for WasmSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.inner)
    }
}

impl Debug for WasmSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.inner.as_ref())
    }
}
