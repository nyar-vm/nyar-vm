use super::*;

impl AsRef<str> for WasmSymbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}
impl<'a> From<&'a str> for WasmSymbol {
    fn from(value: &'a str) -> Self {
        Self { inner: Arc::from(value) }
    }
}

impl From<String> for WasmSymbol {
    fn from(value: String) -> Self {
        Self { inner: Arc::from(value) }
    }
}
impl From<Arc<str>> for WasmSymbol {
    fn from(value: Arc<str>) -> Self {
        Self { inner: value }
    }
}

impl From<&'static str> for WasmExternalName {
    fn from(value: &'static str) -> Self {
        Self { package: None, name: Arc::from(value), version: None }
    }
}

impl FromStr for WasmExternalName {
    type Err = SyntaxError;

    /// `wasi:random/random@0.2.0`
    /// `wasi:random/random`
    /// `random`
    fn from_str(s: &str) -> Result<Self, SyntaxError> {
        match s.split_once('/') {
            Some((package, module)) => {
                let (module, version) = match module.split_once('@') {
                    Some((module, version)) => match Version::parse(version) {
                        Ok(version) => (module, Some(version)),
                        Err(_) => return Err(SyntaxError::new("Invalid semver version")),
                    },
                    None => (module, None),
                };
                match package.split_once(':') {
                    Some((organization, project)) => Ok(WasmExternalName {
                        name: Arc::from(module),
                        package: Some(WasmPublisher { organization: Arc::from(organization), project: Arc::from(project) }),
                        version,
                    }),
                    None => Err(SyntaxError::new("Missing organization name")),
                }
            }
            None => Ok(WasmExternalName { name: Arc::from(s), package: None, version: None }),
        }
    }
}
