use std::str::FromStr;

use nyar_error::SyntaxError;

use super::*;

// impl Default for WasiModule {
//     fn default() -> Self {
//         Self { package: None, name: Arc::from(""), version: None }
//     }
// }

impl From<&str> for WasiModule {
    fn from(value: &str) -> Self {
        Self { name: value.into(), package: None, version: None }
    }
}

impl From<String> for WasiModule {
    fn from(value: String) -> Self {
        Self { name: value.into(), package: None, version: None }
    }
}

impl From<Arc<str>> for WasiModule {
    fn from(value: Arc<str>) -> Self {
        Self { name: value.into(), package: None, version: None }
    }
}

impl FromStr for WasiModule {
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
                    Some((organization, project)) => Ok(Self {
                        name: Arc::from(module),
                        package: Some(WasiPublisher { organization: Arc::from(organization), project: Arc::from(project) }),
                        version,
                    }),
                    None => Err(SyntaxError::new("Missing organization name")),
                }
            }
            None => Ok(Self { name: Arc::from(s), package: None, version: None }),
        }
    }
}
