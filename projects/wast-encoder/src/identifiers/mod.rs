use semver::Version;
use std::sync::Arc;

/// e.g: `wasi:random/random@0.2.0`
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiModule {
    name: string_pool::String,
    package: Option<WasiPublisher>,
    version: Option<Version>,
}

/// e.g.: `wasi:random`
#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiPublisher {
    organization: string_pool::String,
    project: string_pool::String,
}

impl WasiModule {
    /// Create a new module without a publisher
    pub fn create<S>(name: S) -> Self
    where
        S: Into<string_pool::String>,
    {
        Self { package: None, name: name.into(), version: None }
    }
    /// Set the publisher for the module
    pub fn with_publisher(self, publisher: WasiPublisher) -> Self {
        Self { package: Some(publisher), ..self }
    }
    /// Set the version for the module
    pub fn with_version(self, version: Version) -> Self {
        Self { version: Some(version), ..self }
    }
    /// Set the organization and project for the module
    pub fn with_project<O, P>(self, organization: O, project: P) -> Self
    where
        O: Into<string_pool::String>,
        P: Into<string_pool::String>,
    {
        Self { package: Some(WasiPublisher { organization: organization.into(), project: project.into() }), ..self }
    }
}
