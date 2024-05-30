use nyar_types::*;
#[test]
fn provenance_is_required() {
    assert!(Provenance::new("pkg", "fn.return").is_some());
    assert!(Provenance::new("", "fn").is_none());
}
#[test]
fn untraceable_artifacts_fail_closed() {
    let c = ArtifactContract { interface_hash: "i".into(), target: "jvm".into(), artifact_hash: "a".into(), provenance: vec![] };
    assert!(c.verify().is_err());
}
#[test]
fn primitive_identity_is_explicit() {
    let mut r = PrimitiveRegistry::default();
    let d = PrimitiveDefinition {
        identity: "Float64".into(),
        canonical_type: NyarType::Float64,
        provenance: Provenance::new("stdlib", "definition.Float64").unwrap(),
    };
    assert!(r.register(d).is_ok());
}
#[test]
fn evidence_cannot_skip_a_gate() {
    assert!(EvidenceStatus::DiagnosticOnly.can_promote_to(EvidenceStatus::ContractProven));
    assert!(!EvidenceStatus::DiagnosticOnly.can_promote_to(EvidenceStatus::LaneProven));
}
#[test]
fn evidence_requires_complete_lineage() {
    let package = EvidencePackage {
        schema_version: "0".into(),
        interface: SemanticPackageInterface {
            schema_version: "0".into(),
            package: "p".into(),
            source_hash: "s".into(),
            resolution_hash: "r".into(),
            core_surface_version: "c".into(),
            definition_registry_version: "d".into(),
            observations: vec![],
        },
        artifact: ArtifactContract {
            interface_hash: "i".into(),
            target: "clr".into(),
            artifact_hash: "a".into(),
            provenance: vec![Provenance::new("p", "artifact").unwrap()],
        },
        lineage: vec![],
        probe: SemanticObservation { ty: NyarType::Unit, value: "unit".into(), provenance: Provenance::new("p", "probe").unwrap() },
    };
    assert!(package.verify().is_err());
}

#[test]
fn canonical_interface_hash_is_stable() {
    let interface = SemanticPackageInterface {
        schema_version: "0".into(),
        package: "p".into(),
        source_hash: "s".into(),
        resolution_hash: "r".into(),
        core_surface_version: "c".into(),
        definition_registry_version: "d".into(),
        observations: vec![],
    };
    assert_eq!(
        interface.canonical_json(),
        "{\"core_surface_version\":\"c\",\"definition_registry_version\":\"d\",\"observations\":[],\"package\":\"p\",\"resolution_hash\":\"r\",\"schema_version\":\"0\",\"source_hash\":\"s\"}"
    );
    assert_eq!(interface.canonical_hash().len(), 64);
}
