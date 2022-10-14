use super::*;

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

