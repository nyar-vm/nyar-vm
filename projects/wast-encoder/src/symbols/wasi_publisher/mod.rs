use std::{
    fmt::{Debug, Display, Formatter},
    hash::Hash,
    sync::Arc,
};

use semver::Version;

mod convert;
mod display;

/// e.g.: `wasi:random`
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiPublisher {
    organization: Arc<str>,
    project: Arc<str>,
}

/// e.g: `wasi:random/random@0.2.0`
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiModule {
    pub package: Option<WasiPublisher>,
    pub name: Arc<str>,
    pub version: Option<Version>,
}

impl WasiModule {
    /// Create a new module without a publisher
    pub fn create<S>(name: S) -> Self
    where
        S: Into<Arc<str>>,
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
        O: Into<Arc<str>>,
        P: Into<Arc<str>>,
    {
        Self { package: Some(WasiPublisher { organization: organization.into(), project: project.into() }), ..self }
    }
}
