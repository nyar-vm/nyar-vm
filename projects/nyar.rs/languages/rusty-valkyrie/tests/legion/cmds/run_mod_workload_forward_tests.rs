//! Migrated from `src/cmds/run/mod.rs` (`workload_forward_tests`).

use rusty_valkyrie::legion::cmds::run::*;

use nyar::{CanonicalTarget, RunnerFamily};
use rusty_valkyrie::legion::workspace::planner::BuildPlan;
use std::path::{Path, PathBuf};

fn bare_run_args() -> RunArgs {
    RunArgs {
        project_dir: PathBuf::from("."),
        target: CanonicalTarget::parse("ovm").expect("ovm"),
        output_dir: None,
        runner: Vec::new(),
        artifact: None,
        dry_run: true,
        debug_artifacts: false,
        workload_json: None,
        workload_file: None,
    }
}

#[test]
fn appends_workload_json_for_nyar_vm() {
    let mut command =
        RunCommand { target: RunnerFamily::Ovm, artifact: PathBuf::from("out.odin"), command: "odin-vm".into(), args: vec!["out.odin".into()] };
    let mut args = bare_run_args();
    args.workload_json = Some(r#"{"pause_budget_ms":5}"#.into());
    append_ovm_workload_args(&mut command, &args).expect("append");
    assert!(command.args.windows(2).any(|w| w[0] == "--workload-json" && w[1].contains("pause_budget_ms")));
}

#[test]
fn resolver_compile_evidence_matches_compiler_helpers() {
    let groups = vec![nyar_types::compiler_input::CompilerSourceGroup::single_source("app", "app", "[main] micro main() -> i32 { return 1 }")];
    let digest = expected_resolver_compile_evidence_from_groups(&groups, &[], "ovm");
    assert_eq!(digest.semantic_closure_hash, rusty_valkyrie::semantic_closure_hash_for_groups(&groups, "ovm"));
    assert_eq!(digest.host_provider_binding_hash, rusty_valkyrie::host_provider_binding_hash_for_bindings(&[]));
}

#[test]
fn is_fresh_rejects_stale_host_provider_compile_evidence() {
    use rusty_valkyrie::legion::workspace::{
        manifest::ProjectArtifactKind,
        planner::{BuildPlan, PlannedHostProvider, PlannedProject, PlannedSemanticSourceGroup},
    };
    use std::fs;
    use tempfile::tempdir;

    let directory = tempdir().expect("tempdir");
    let root = directory.path();
    let source_path = root.join("main.v");
    fs::write(&source_path, "[main] micro main() -> i32 { return 1 }").expect("write source");
    let manifest_path = root.join("legion.von");
    fs::write(&manifest_path, "name: demo\n").expect("write manifest");
    let group = PlannedSemanticSourceGroup {
        dependency_key: "demo".into(),
        name: "demo".into(),
        manifest_dir: root.to_path_buf(),
        source_files: vec![source_path.clone()],
        direct_dependencies: Vec::new(),
        is_entry: true,
    };
    let target = CanonicalTarget::parse("ovm").expect("ovm");
    let plan = BuildPlan {
        workspace_root: root.to_path_buf(),
        output_dir: root.join("dist"),
        enabled_features: Default::default(),
        project: PlannedProject {
            name: "demo".into(),
            manifest_dir: root.to_path_buf(),
            manifest_path: manifest_path.clone(),
            source_files: vec![source_path.clone()],
            semantic_source_groups: vec![group],
            host_contracts: Vec::new(),
            host_provider_candidates: Vec::new(),
            selected_host_providers: vec![PlannedHostProvider {
                contract: "demo::write".into(),
                contract_key: Some(micro_declaration("demo", "demo.v", "write")),
                symbol: "std::write".into(),
                key: micro_declaration("std", "std.v", "write"),
                source_file: manifest_path.clone(),
                line: 1,
                required_capabilities: Vec::new(),
                foreign_bindings: Vec::new(),
                when_attributes: Vec::new(),
                gate_attributes: Vec::new(),
            }],
            build_target: rusty_valkyrie::legion::workspace::manifest::BuildTargetSpec { target, ..Default::default() },
            artifact_kind: ProjectArtifactKind::Binary,
            dependencies: Vec::new(),
            gated_out_host_providers: Vec::new(),
            is_test: false,
            test_entries: Vec::new(),
        },
    };
    fs::create_dir_all(&plan.output_dir).expect("output dir");
    let artifact_path = plan.output_dir.join("app.odin");
    fs::write(&artifact_path, b"nyar-artifact-stub").expect("artifact");
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let inputs = collect_execution_input_digests(&plan).expect("input digests");
    let manifest = ExecutionManifest {
        schema_version: EXECUTION_MANIFEST_SCHEMA_VERSION,
        identity_schema_version: nyar_types::IDENTITY_SCHEMA_VERSION,
        mir_contract_version: nyar_types::MIR_CONTRACT_VERSION,
        layout_plan_version: nyar_types::LAYOUT_PLAN_VERSION,
        bytecode_format_version: acorn_odin::BYTECODE_FORMAT_VERSION,
        project_name: plan.project.name.clone(),
        target: plan.project.build_target.target.to_string(),
        artifact_kind: plan.project.artifact_kind,
        library_exports: Vec::new(),
        inputs,
        artifacts: vec![ExecutionArtifactDigest { path: "app.odin".into(), hash: hash_file_sha256(&artifact_path).expect("artifact hash") }],
        run_contracts: vec![RunContract {
            logical_entry: "main".into(),
            physical_entry: "app.odin".into(),
            invocation: "odin-vm app.odin".into(),
            validate: "exists".into(),
        }],
        compile_evidence: Some(CompileEvidenceDigest {
            semantic_closure_hash: expected.semantic_closure_hash.clone(),
            canonical_surface_hash: String::new(),
            host_provider_binding_hash: "stale-host-provider-hash".into(),
            generated_source_hash: String::new(),
        }),
        toolchain_fingerprint: rusty_valkyrie::legion::cache::toolchain_fingerprint(),
    };
    assert!(!manifest.is_fresh_for_plan(&plan).expect("freshness check"), "stale host provider evidence must invalidate execution manifest");
}

fn write_test_compile_evidence(output_dir: &Path, digest: &CompileEvidenceDigest) {
    use std::fs;

    let evidence = rusty_valkyrie::compiler::emitter::pipeline::CompilerCompileEvidence {
        semantic_closure_hash: digest.semantic_closure_hash.clone(),
        canonical_surface_hash: digest.canonical_surface_hash.clone(),
        host_provider_binding_hash: digest.host_provider_binding_hash.clone(),
        host_provider_bindings: Vec::new(),
        generated_source_hash: digest.generated_source_hash.clone(),
    };
    let content = rusty_valkyrie::legion::write_von_indented(&evidence).expect("serialize compile evidence");
    fs::write(output_dir.join(COMPILE_EVIDENCE_FILE_NAME), content).expect("write compile evidence");
}

fn sample_freshness_plan() -> (tempfile::TempDir, BuildPlan, PathBuf) {
    use rusty_valkyrie::legion::workspace::{
        manifest::ProjectArtifactKind,
        planner::{BuildPlan, PlannedProject, PlannedSemanticSourceGroup},
    };
    use std::fs;
    use tempfile::tempdir;

    let directory = tempdir().expect("tempdir");
    let root = directory.path();
    let source_path = root.join("main.v");
    fs::write(&source_path, "[main] micro main() -> i32 { return 1 }").expect("write source");
    let manifest_path = root.join("legion.von");
    fs::write(&manifest_path, "name: demo\n").expect("write manifest");
    let group = PlannedSemanticSourceGroup {
        dependency_key: "demo".into(),
        name: "demo".into(),
        manifest_dir: root.to_path_buf(),
        source_files: vec![source_path.clone()],
        direct_dependencies: Vec::new(),
        is_entry: true,
    };
    let target = CanonicalTarget::parse("ovm").expect("ovm");
    let plan = BuildPlan {
        workspace_root: root.to_path_buf(),
        output_dir: root.join("dist"),
        enabled_features: Default::default(),
        project: PlannedProject {
            name: "demo".into(),
            manifest_dir: root.to_path_buf(),
            manifest_path: manifest_path.clone(),
            source_files: vec![source_path],
            semantic_source_groups: vec![group],
            host_contracts: Vec::new(),
            host_provider_candidates: Vec::new(),
            selected_host_providers: Vec::new(),
            build_target: rusty_valkyrie::legion::workspace::manifest::BuildTargetSpec { target, ..Default::default() },
            artifact_kind: ProjectArtifactKind::Binary,
            dependencies: Vec::new(),
            gated_out_host_providers: Vec::new(),
            is_test: false,
            test_entries: Vec::new(),
        },
    };
    fs::create_dir_all(&plan.output_dir).expect("output dir");
    let artifact_path = plan.output_dir.join("app.odin");
    fs::write(&artifact_path, b"nyar-artifact-stub").expect("artifact");
    (directory, plan, artifact_path)
}

fn sample_execution_manifest(plan: &BuildPlan, artifact_path: &Path, evidence: CompileEvidenceDigest) -> ExecutionManifest {
    ExecutionManifest {
        schema_version: EXECUTION_MANIFEST_SCHEMA_VERSION,
        identity_schema_version: nyar_types::IDENTITY_SCHEMA_VERSION,
        mir_contract_version: nyar_types::MIR_CONTRACT_VERSION,
        layout_plan_version: nyar_types::LAYOUT_PLAN_VERSION,
        bytecode_format_version: acorn_odin::BYTECODE_FORMAT_VERSION,
        project_name: plan.project.name.clone(),
        target: plan.project.build_target.target.to_string(),
        artifact_kind: plan.project.artifact_kind,
        library_exports: Vec::new(),
        inputs: collect_execution_input_digests(plan).expect("input digests"),
        artifacts: vec![ExecutionArtifactDigest { path: "app.odin".into(), hash: hash_file_sha256(artifact_path).expect("artifact hash") }],
        run_contracts: vec![RunContract {
            logical_entry: "main".into(),
            physical_entry: "app.odin".into(),
            invocation: "odin-vm app.odin".into(),
            validate: "exists".into(),
        }],
        compile_evidence: Some(evidence),
        toolchain_fingerprint: rusty_valkyrie::legion::cache::toolchain_fingerprint(),
    }
}

#[test]
fn is_fresh_rejects_stale_semantic_closure_compile_evidence() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let evidence = CompileEvidenceDigest {
        semantic_closure_hash: "stale-semantic-closure-hash".into(),
        canonical_surface_hash: "canonical-surface-digest".into(),
        host_provider_binding_hash: expected.host_provider_binding_hash,
        generated_source_hash: String::new(),
    };
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    assert!(!manifest.is_fresh_for_plan(&plan).expect("freshness check"), "stale semantic closure evidence must invalidate execution manifest");
}

#[test]
fn is_fresh_accepts_matching_compile_evidence_sidecar() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let evidence = CompileEvidenceDigest {
        semantic_closure_hash: expected.semantic_closure_hash,
        canonical_surface_hash: "canonical-surface-digest".into(),
        host_provider_binding_hash: expected.host_provider_binding_hash,
        generated_source_hash: String::new(),
    };
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    assert!(
        manifest.is_fresh_for_plan(&plan).expect("freshness check"),
        "matching compile-evidence sidecar must keep execution manifest fresh"
    );
}

#[test]
fn is_fresh_rejects_mismatched_compile_evidence_sidecar() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let evidence = CompileEvidenceDigest {
        semantic_closure_hash: expected.semantic_closure_hash,
        canonical_surface_hash: "canonical-surface-digest".into(),
        host_provider_binding_hash: expected.host_provider_binding_hash,
        generated_source_hash: String::new(),
    };
    write_test_compile_evidence(
        &plan.output_dir,
        &CompileEvidenceDigest { canonical_surface_hash: "different-canonical-surface".into(), ..evidence.clone() },
    );
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    assert!(
        !manifest.is_fresh_for_plan(&plan).expect("freshness check"),
        "mismatched compile-evidence sidecar must invalidate execution manifest"
    );
}

fn matching_evidence(plan: &BuildPlan) -> CompileEvidenceDigest {
    let expected = expected_resolver_compile_evidence(plan).expect("expected evidence");
    CompileEvidenceDigest {
        semantic_closure_hash: expected.semantic_closure_hash,
        canonical_surface_hash: "canonical-surface-digest".into(),
        host_provider_binding_hash: expected.host_provider_binding_hash,
        generated_source_hash: String::new(),
    }
}

fn stale_reason(manifest: &ExecutionManifest, output_dir: &Path) -> String {
    let verdict = manifest.verify_against_output_dir(output_dir).expect("verify");
    match verdict {
        ManifestVerdict::Stale(reason) => reason,
        other => panic!("expected stale verdict, got {other:?}"),
    }
}

#[test]
fn verify_output_accepts_self_consistent_manifest_only_at_artifact_level() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    manifest.write_to_output_dir(&plan.output_dir).expect("write manifest");
    let reread = ExecutionManifest::read_from_output_dir(&plan.output_dir).expect("read manifest").expect("manifest present");
    assert_eq!(reread.toolchain_fingerprint, rusty_valkyrie::legion::cache::toolchain_fingerprint(), "toolchain fingerprint must round-trip through VON");
    // 无当前 workspace 输入上下文时，自洽 manifest 最高只到"仅产物自洽"，不得给出可复用的 fresh。
    assert_eq!(
        reread.verify_against_output_dir(&plan.output_dir).expect("verify"),
        ManifestVerdict::Verified(ManifestVerificationLevel::ArtifactSelfConsistent),
        "plan-free verification must cap at artifact-self-consistent"
    );
    assert!(!reread.verify_against_output_dir(&plan.output_dir).expect("verify").is_fully_verified(), "artifact-self-consistent is not fully verified");
}

#[test]
fn verify_with_plan_context_reaches_fully_verified() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    let inputs = collect_execution_input_digests(&plan).expect("input digests");
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let target = plan.project.build_target.target.to_string();
    let context = ManifestVerificationContext {
        expected_project: Some((plan.project.name.as_str(), target.as_str(), plan.project.artifact_kind)),
        current_inputs: Some(&inputs),
        expected_evidence: Some(&expected),
    };
    assert_eq!(
        manifest.verify(&plan.output_dir, &context).expect("verify"),
        ManifestVerdict::Verified(ManifestVerificationLevel::FullyVerified),
        "full workspace input context must reach fully-verified"
    );
}

#[test]
fn verify_rejects_manifest_from_different_compiler_build() {
    // 同 manifest 换构建身份（dirty 内容变化会改变 compiler= 段）→ stale。
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    let inputs = collect_execution_input_digests(&plan).expect("input digests");
    let expected = expected_resolver_compile_evidence(&plan).expect("expected evidence");
    let target = plan.project.build_target.target.to_string();
    let context = ManifestVerificationContext {
        expected_project: Some((plan.project.name.as_str(), target.as_str(), plan.project.artifact_kind)),
        current_inputs: Some(&inputs),
        expected_evidence: Some(&expected),
    };
    let current = rusty_valkyrie::legion::cache::toolchain_fingerprint();
    let other_build = ExecutionManifest {
        toolchain_fingerprint: format!("{current};compiler=ffffffffffff-dirty0123456"),
        ..manifest.clone()
    };
    let verdict = other_build.verify(&plan.output_dir, &context).expect("verify");
    let reason = verdict.stale_reason().unwrap_or_else(|| panic!("different compiler build must be stale, got {verdict:?}"));
    assert!(reason.contains("toolchain fingerprint"), "unexpected reason: {reason}");
    assert!(!other_build.is_fresh_for_plan(&plan).expect("freshness check"), "is_fresh_for_plan must reject foreign compiler build identity");
}

#[test]
fn verify_output_dir_rejects_tampered_artifact_bytes() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    std::fs::write(&artifact_path, b"tampered-artifact-bytes").expect("tamper artifact");
    let reason = stale_reason(&manifest, &plan.output_dir);
    assert!(reason.contains("digest mismatch"), "unexpected reason: {reason}");
}

#[test]
fn verify_output_dir_rejects_missing_artifact_file() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    std::fs::remove_file(&artifact_path).expect("remove artifact");
    let reason = stale_reason(&manifest, &plan.output_dir);
    assert!(reason.contains("missing from output dir"), "unexpected reason: {reason}");
}

#[test]
fn verify_output_dir_rejects_missing_or_stale_toolchain_fingerprint() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let base = sample_execution_manifest(&plan, &artifact_path, evidence);

    let missing = ExecutionManifest { toolchain_fingerprint: String::new(), ..base.clone() };
    let reason = stale_reason(&missing, &plan.output_dir);
    assert!(reason.contains("missing toolchain fingerprint"), "unexpected reason: {reason}");

    let stale = ExecutionManifest { toolchain_fingerprint: "legion=0.0.0;compiler=deadbeef".into(), ..base };
    let reason = stale_reason(&stale, &plan.output_dir);
    assert!(reason.contains("toolchain fingerprint"), "unexpected reason: {reason}");
}

#[test]
fn unknown_compiler_build_identity_is_not_reusable() {
    // 无 Git 构建的 `unknown` 身份必须被权威核心判不可复用（正式复用拒绝）。
    assert!(untrusted_compiler_identity_reason("unknown").is_some(), "`unknown` identity must be refused");
    assert!(untrusted_compiler_identity_reason("").is_some(), "empty identity must be refused");
    // 裸 `-dirty`（无内容摘要）不区分同修订的不同未提交内容，一律拒绝可信复用。
    assert!(untrusted_compiler_identity_reason("efbc884c1234-dirty").is_some(), "bare `-dirty` suffix without digest must be refused");
    // 正常路径：dirty 身份必须绑定内容摘要，绑定后才可信。
    assert!(
        untrusted_compiler_identity_reason("efbc884c1234-dirty0123456789").is_none(),
        "git-derived dirty identity with content digest is acceptable"
    );
    assert!(untrusted_compiler_identity_reason("efbc884c1234").is_none(), "clean git revision identity is acceptable");
    // 运行时读取当前构建身份：正式复用环境（cargo/git 检出）不得是 `unknown`。
    let current = rusty_valkyrie::legion::cache::compiler_build_id();
    assert!(!current.is_empty(), "compiler build id must be present in a git checkout");
    if untrusted_compiler_identity_reason(current).is_none() {
        assert!(rusty_valkyrie::legion::cache::toolchain_fingerprint().contains(&format!("compiler={current}")));
    }
}

#[test]
fn verify_with_plan_context_rejects_changed_sources() {
    // 源码变化：manifest 记录的输入摘要与当前 workspace 重算不一致 → 不可信复用。
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let manifest = sample_execution_manifest(&plan, &artifact_path, evidence);
    let stale_inputs = manifest.inputs.clone();
    std::fs::write(plan.project.source_files.first().expect("source file"), "// edited after build\n").expect("edit source");
    let current_inputs = collect_execution_input_digests(&plan).expect("recomputed input digests");
    assert_ne!(stale_inputs, current_inputs, "source edit must change recomputed input digests");
    let target = plan.project.build_target.target.to_string();
    let context = ManifestVerificationContext {
        expected_project: Some((plan.project.name.as_str(), target.as_str(), plan.project.artifact_kind)),
        current_inputs: Some(&current_inputs),
        expected_evidence: Some(&expected_resolver_compile_evidence(&plan).expect("expected evidence")),
    };
    let verdict = manifest.verify(&plan.output_dir, &context).expect("verify");
    let reason = verdict.stale_reason().unwrap_or_else(|| panic!("changed sources must invalidate reuse, got {verdict:?}"));
    assert!(reason.contains("input digests differ"), "unexpected reason: {reason}");
    assert!(!manifest.is_fresh_for_plan(&plan).expect("freshness check"), "changed sources must not be reusable");
}

#[test]
fn verify_output_dir_rejects_missing_evidence_fields_and_sidecar() {
    let (_directory, plan, artifact_path) = sample_freshness_plan();
    let evidence = matching_evidence(&plan);
    write_test_compile_evidence(&plan.output_dir, &evidence);
    let base = sample_execution_manifest(&plan, &artifact_path, evidence);

    let no_evidence = ExecutionManifest { compile_evidence: None, ..base.clone() };
    let reason = stale_reason(&no_evidence, &plan.output_dir);
    assert!(reason.contains("missing compile_evidence"), "unexpected reason: {reason}");

    let no_inputs = ExecutionManifest { inputs: Vec::new(), ..base.clone() };
    let reason = stale_reason(&no_inputs, &plan.output_dir);
    assert!(reason.contains("input digests"), "unexpected reason: {reason}");

    let no_artifacts = ExecutionManifest { artifacts: Vec::new(), ..base.clone() };
    let reason = stale_reason(&no_artifacts, &plan.output_dir);
    assert!(reason.contains("artifact digests"), "unexpected reason: {reason}");

    let (_directory2, plan2, artifact_path2) = sample_freshness_plan();
    let evidence2 = matching_evidence(&plan2);
    let manifest2 = sample_execution_manifest(&plan2, &artifact_path2, evidence2);
    let reason = stale_reason(&manifest2, &plan2.output_dir);
    assert!(reason.contains("compile evidence sidecar"), "unexpected reason: {reason}");
}

#[test]
fn resolver_compile_evidence_host_provider_hash_is_binding_sensitive() {
    let groups = vec![nyar_types::compiler_input::CompilerSourceGroup::single_source("app", "app", "[main] micro main() -> i32 { return 1 }")];
    let first = rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
        contract: micro_declaration("demo", "demo.v", "write"),
        provider: micro_declaration("std", "std.v", "write"),
        required_capabilities: vec!["js.fetch".into()],
        foreign_bindings: vec![r#"js("fetch")"#.into()],
    };
    let second = rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
        contract: micro_declaration("demo", "demo.v", "write"),
        provider: micro_declaration("std", "std.v", "print"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    };
    let without = expected_resolver_compile_evidence_from_groups(&groups, &[], "ovm");
    let with_first = expected_resolver_compile_evidence_from_groups(&groups, &[first.clone()], "ovm");
    let with_second = expected_resolver_compile_evidence_from_groups(&groups, &[second], "ovm");
    assert_eq!(without.semantic_closure_hash, with_first.semantic_closure_hash);
    assert_ne!(with_first.host_provider_binding_hash, without.host_provider_binding_hash);
    assert_ne!(with_first.host_provider_binding_hash, with_second.host_provider_binding_hash);
}

#[test]
fn rejects_workload_json_for_non_ovm() {
    let mut command =
        RunCommand { target: RunnerFamily::Node, artifact: PathBuf::from("out.mjs"), command: "node".into(), args: vec!["out.mjs".into()] };
    let mut args = bare_run_args();
    args.workload_json = Some("{}".into());
    let err = append_ovm_workload_args(&mut command, &args).expect_err("must reject");
    assert!(err.to_string().contains("ovm"));
}

fn micro_declaration(dependency_key: &str, source_path: &str, name: &str) -> rusty_valkyrie::compiler::semantic::closure::DeclarationKey {
    rusty_valkyrie::compiler::semantic::closure::DeclarationKey {
        dependency_key: dependency_key.to_owned(),
        source_path: source_path.to_owned(),
        namespace: Vec::new(),
        kind: rusty_valkyrie::DeclarationKind::Micro,
        owner: None,
        name: name.to_owned(),
    }
}


