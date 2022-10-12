use std::{
    cmp::Ordering,
    collections::BTreeMap,
    fmt::{Display, Formatter, Write},
    hash::{Hash, Hasher},
    sync::Arc,
};

use semver::Version;

use crate::encoder::WastEncoder;

mod display;

pub struct WasiLinker {
    packages: BTreeMap<WasiPublisher, Version>,
}

/// e.g: `wasi:random/random@0.2.0`
#[derive(Clone)]
pub struct WasiModule {
    pub name: string_pool::String,
    pub package: Option<WasiPublisher>,
    pub version: Option<Version>,
}

impl Hash for WasiModule {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.package.hash(state);
        self.version.hash(state);
    }
}

impl Eq for WasiModule {}

impl PartialEq for WasiModule {
    fn eq(&self, other: &Self) -> bool {
        self.name.as_str().eq(other.name.as_str()) && self.package == other.package && self.version == other.version
    }
}

impl PartialOrd for WasiModule {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for WasiModule {
    fn cmp(&self, other: &Self) -> Ordering {
        self.package
            .cmp(&other.package)
            .then_with(|| self.name.as_str().cmp(&other.name.as_str()))
            .then_with(|| self.version.cmp(&other.version))
    }
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
            O: Into<Arc<str>>,
            P: Into<Arc<str>>,
    {
        Self { package: Some(WasiPublisher { organization: organization.into(), project: project.into() }), ..self }
    }
}

pub struct WasiInstance {
    module: WasiModule,
    /// language_name: wasi_name
    resources: BTreeMap<Arc<str>, Arc<str>>,
}

impl From<&str> for WasiModule {
    fn from(value: &str) -> Self {
        Self { name: value.into(), package: None, version: None }
    }
}

impl WasiInstance {
    pub fn new<M>(module: M) -> Self
        where
            M: Into<WasiModule>,
    {
        Self { module: module.into(), resources: Default::default() }
    }
    pub fn add_resource<S, T>(&mut self, language_name: S, wasi_name: T)
        where
            S: Into<Arc<str>>,
            T: Into<Arc<str>>,
    {
        self.resources.insert(language_name.into(), wasi_name.into());
    }
}
