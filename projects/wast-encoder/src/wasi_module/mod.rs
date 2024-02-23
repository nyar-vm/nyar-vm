use crate::encoder::WastEncoder;
use semver::Version;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fmt::{Display, Formatter, Write},
    hash::{Hash, Hasher},
};

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
#[derive(Debug, Clone)]
pub struct WasiPublisher {
    organization: string_pool::String,
    project: string_pool::String,
}

impl PartialEq for WasiPublisher {
    fn eq(&self, other: &Self) -> bool {
        self.organization.as_str() == other.organization.as_str() && self.project.as_str() == other.project.as_str()
    }
}

impl Ord for WasiPublisher {
    fn cmp(&self, other: &Self) -> Ordering {
        self.organization
            .as_str()
            .cmp(other.organization.as_str())
            .then_with(|| self.project.as_str().cmp(other.project.as_str()))
    }
}

impl PartialOrd for WasiPublisher {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for WasiPublisher {}

impl Hash for WasiPublisher {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.organization.hash(state);
        self.project.hash(state);
    }
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

pub struct WasiInstance {
    module: WasiModule,
}

pub enum WasiImport {
    Instance(WasiInstance),
    Module(WasiModule),
}
