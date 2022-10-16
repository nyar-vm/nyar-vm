use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
    hash::Hash,
    ops::AddAssign,
    sync::Arc,
};

use semver::Version;

use crate::{WasiFunction, WasiResource};

mod convert;
mod display;

pub struct WasiLinker {
    packages: BTreeMap<WasiPublisher, Version>,
}

/// e.g: `wasi:random/random@0.2.0`
#[derive(Clone, Default, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiModule {
    pub package: Option<WasiPublisher>,
    pub name: Arc<str>,
    pub version: Option<Version>,
}

/// e.g.: `wasi:random`
#[derive(Clone, Debug, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct WasiPublisher {
    organization: Arc<str>,
    project: Arc<str>,
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

pub struct WasiInstance {
    pub module: WasiModule,
    /// language_name: wasi_name
    pub resources: BTreeMap<Arc<str>, WasiResource>,
    pub functions: BTreeMap<Arc<str>, WasiFunction>,
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
    where
        M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default(), functions: Default::default() }
    }
}
