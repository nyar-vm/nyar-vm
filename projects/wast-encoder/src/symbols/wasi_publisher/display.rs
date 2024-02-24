use super::*;

impl Debug for WasiModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let debug = &mut f.debug_struct("WasiModule");
        if let Some(s) = &self.package {
            debug.field("organization", &s.organization);
            debug.field("project", &s.project);
        }
        debug.field("component", &self.name);
        if let Some(s) = &self.version {
            debug.field("version", &s.to_string());
        }
        debug.finish()
    }
}

impl Display for WasiModule {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(s) = &self.package {
            write!(f, "{}/", s)?
        }
        write!(f, "{}", self.name)?;
        if let Some(s) = &self.version {
            write!(f, "@{}", s)?
        }
        Ok(())
    }
}

impl Display for WasiPublisher {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.organization, self.project)
    }
}
