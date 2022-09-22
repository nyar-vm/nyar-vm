use super::*;

impl Debug for WasmSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.inner.as_ref())
    }
}

impl Display for WasmSymbol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Debug for WasmExternalName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.module, f)
    }
}

impl Display for WasmPublisher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.organization, self.project)
    }
}

impl Display for WasmExternalName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.package {
            write!(f, "{}/", s)?
        }
        write!(f, "{}", self.module)?;
        if let Some(s) = &self.version {
            write!(f, "@{}", s)?
        }
        Ok(())
    }
}

impl WasmExternalName {
    /// Get export name and erase lifetime
    pub fn as_ref(&self) -> &str {
        unsafe {
            let fake = self.to_string();
            let ptr = fake.as_str() as *const str;
            &*ptr
        }
    }
}
