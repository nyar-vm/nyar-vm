//! Narrow contracts for proving semantic and backend boundaries.
//!
//! These types intentionally contain provenance and verification state, but no
//! backend stack/runtime state. An observation without provenance is invalid.

use crate::NyarType;
use sha2::{Digest, Sha256};

/// Versioned identity of a package semantic interface.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SemanticPackageInterface {
    /// Contract schema version.
    pub schema_version: String,
    /// Package identity.
    pub package: String,
    /// Canonical source hash.
    pub source_hash: String,
    /// Canonical dependency resolution hash.
    pub resolution_hash: String,
    /// Core semantic surface version.
    pub core_surface_version: String,
    /// Primitive/definition registry version.
    pub definition_registry_version: String,
    /// Exported semantic observations.
    pub observations: Vec<SemanticObservation>,
}

/// A stable source/contract location that explains where a value came from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Provenance {
    /// Package-relative source identity or contract transformation id.
    pub source: String,
    /// Stable semantic field/path within the source or contract.
    pub path: String,
}

impl Provenance {
    /// Creates a provenance record, rejecting empty attribution fields.
    pub fn new(source: impl Into<String>, path: impl Into<String>) -> Option<Self> {
        let record = Self { source: source.into(), path: path.into() };
        (!record.source.trim().is_empty() && !record.path.trim().is_empty()).then_some(record)
    }
}

/// A neutral observable result used by reference semantics and runtime probes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SemanticObservation {
    /// Canonical result type; never inferred from a host representation.
    pub ty: NyarType,
    /// Canonical observable value.
    pub value: String,
    /// Source/contract attribution for the observation.
    pub provenance: Provenance,
}

impl SemanticObservation {
    /// Rejects observations that lack a non-empty value or provenance.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.value.is_empty() {
            return Err("semantic observation value is empty");
        }
        if self.provenance.source.trim().is_empty() || self.provenance.path.trim().is_empty() {
            return Err("semantic observation provenance is incomplete");
        }
        Ok(())
    }
}

impl SemanticPackageInterface {
    /// Verifies identity and field provenance.
    pub fn verify(&self) -> Result<(), &'static str> {
        let fields = [
            self.schema_version.as_str(),
            self.package.as_str(),
            self.source_hash.as_str(),
            self.resolution_hash.as_str(),
            self.core_surface_version.as_str(),
            self.definition_registry_version.as_str(),
        ];
        if fields.iter().any(|field| field.trim().is_empty()) {
            return Err("semantic package interface identity is incomplete");
        }
        if self.observations.iter().any(|observation| observation.validate().is_err()) {
            return Err("semantic package interface contains untraceable observation");
        }
        Ok(())
    }

    /// Stable, dependency-free canonical JSON representation.
    pub fn canonical_json(&self) -> String {
        format!(
            "{{\"core_surface_version\":\"{}\",\"definition_registry_version\":\"{}\",\"observations\":{},\"package\":\"{}\",\"resolution_hash\":\"{}\",\"schema_version\":\"{}\",\"source_hash\":\"{}\"}}",
            escape(&self.core_surface_version),
            escape(&self.definition_registry_version),
            observations_json(&self.observations),
            escape(&self.package),
            escape(&self.resolution_hash),
            escape(&self.schema_version),
            escape(&self.source_hash)
        )
    }

    /// SHA-256 hash of the canonical interface representation.
    pub fn canonical_hash(&self) -> String {
        sha256_hex(self.canonical_json().as_bytes())
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}
fn observations_json(values: &[SemanticObservation]) -> String {
    let entries = values
        .iter()
        .map(|value| {
            format!(
                "{{\"provenance\":{{\"path\":\"{}\",\"source\":\"{}\"}},\"type\":\"{}\",\"value\":\"{}\"}}",
                escape(&value.provenance.path),
                escape(&value.provenance.source),
                escape(&value.ty.to_string()),
                escape(&value.value)
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", entries.join(","))
}
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Backend-neutral artifact contract. Runtime executors consume only this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ArtifactContract {
    /// Canonical semantic interface hash.
    pub interface_hash: String,
    /// Target executor family (clr, jvm, node-wasm, wasi-component).
    pub target: String,
    /// Artifact content hash.
    pub artifact_hash: String,
    /// Every generated field must retain an attribution record.
    pub provenance: Vec<Provenance>,
}

impl ArtifactContract {
    /// Fails closed when hashes, target, or attribution are missing.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.interface_hash.trim().is_empty() || self.artifact_hash.trim().is_empty() {
            return Err("artifact hashes are incomplete");
        }
        if self.target.trim().is_empty() {
            return Err("artifact target is empty");
        }
        if self.provenance.is_empty() || self.provenance.iter().any(|p| p.source.trim().is_empty() || p.path.trim().is_empty()) {
            return Err("artifact provenance is incomplete");
        }
        Ok(())
    }
}

/// One immutable compiler/executor stage in a bootstrap lineage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BootstrapStage {
    /// Stage label: seed, v1, or v2.
    pub label: String,
    /// Compiler/source hash.
    pub compiler_hash: String,
    /// Produced compiler/artifact hash.
    pub output_hash: String,
    /// Executor identity and version.
    pub executor: String,
    /// Stage provenance.
    pub provenance: Vec<Provenance>,
}

/// Atomic evidence package for one target lane.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct EvidencePackage {
    /// Evidence schema version.
    pub schema_version: String,
    /// Semantic interface evidence.
    pub interface: SemanticPackageInterface,
    /// Artifact contract evidence.
    pub artifact: ArtifactContract,
    /// Ordered seed/v1/v2 stages.
    pub lineage: Vec<BootstrapStage>,
    /// Final neutral probe observation.
    pub probe: SemanticObservation,
}

impl EvidencePackage {
    /// Fail-closed verification of all required evidence.
    pub fn verify(&self) -> Result<(), &'static str> {
        if self.schema_version.trim().is_empty() || self.lineage.len() != 3 {
            return Err("evidence schema or seed/v1/v2 lineage is incomplete");
        }
        self.interface.verify()?;
        self.artifact.verify()?;
        self.probe.validate()?;
        for (stage, expected) in self.lineage.iter().zip(["seed", "v1", "v2"]) {
            if stage.label != expected
                || stage.compiler_hash.trim().is_empty()
                || stage.output_hash.trim().is_empty()
                || stage.executor.trim().is_empty()
                || stage.provenance.is_empty()
            {
                return Err("evidence lineage stage is incomplete");
            }
        }
        Ok(())
    }
}

/// Evidence state; transitions are deliberately monotonic and atomic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceStatus {
    /// Static or diagnostic evidence only.
    DiagnosticOnly,
    /// Neutral contract verification passed.
    ContractProven,
    /// One backend lane completed link, verify and probe.
    LaneProven,
    /// Full seed/v1/v2 chain and all backend lanes passed.
    BootstrapProven,
}

impl EvidenceStatus {
    /// Returns whether a status transition is a single monotonic promotion.
    pub fn can_promote_to(self, next: Self) -> bool {
        next > self && (next as u8) == (self as u8 + 1)
    }
}

/// A canonical primitive definition entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveDefinition {
    /// Stable primitive identity.
    pub identity: String,
    /// Canonical neutral type.
    pub canonical_type: NyarType,
    /// Source/registry attribution.
    pub provenance: Provenance,
}

/// Registry that prevents structural types from acquiring primitive identity.
#[derive(Debug, Clone, Default)]
pub struct PrimitiveRegistry {
    definitions: Vec<PrimitiveDefinition>,
}

impl PrimitiveRegistry {
    /// Registers a primitive only with explicit identity and provenance.
    pub fn register(&mut self, definition: PrimitiveDefinition) -> Result<(), &'static str> {
        if definition.identity.trim().is_empty() {
            return Err("primitive identity is empty");
        }
        if definition.provenance.source.trim().is_empty() || definition.provenance.path.trim().is_empty() {
            return Err("primitive provenance is incomplete");
        }
        if self.definitions.iter().any(|existing| existing.identity == definition.identity) {
            return Err("duplicate primitive identity");
        }
        self.definitions.push(definition);
        Ok(())
    }
    /// Finds a canonical primitive by identity.
    pub fn get(&self, identity: &str) -> Option<&PrimitiveDefinition> {
        self.definitions.iter().find(|d| d.identity == identity)
    }
}
