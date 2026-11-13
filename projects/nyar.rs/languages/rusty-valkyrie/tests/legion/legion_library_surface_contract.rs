//! `artifact: library` 导出面合同：全 lane 必须按 `[export]` 提供可调用导出，
//! 无导出面时结构化失败且不产生 run contract。
//!
//! 正例：两个 `[export]` 的 library，分别按导出名真调用并断言返回值
//! （wasm 走 Node glue `callExport`，OVM 走 odin-vm `run --entry` 等价入口）。
//! 反例：无 `[export]` 的 library 在 wasm / ovm / jvm / clr 四条 lane 各自失败。

use std::{fs, path::PathBuf, process::ExitCode};

use nyar::ArtifactKind;
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::{
        build::{BuildArgs, compile_plan, run},
        run::ExecutionManifest,
    },
    workspace::planner::{BuildRequest, WorkspaceResolver},
};
use tempfile::TempDir;

const PRIMITIVES: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")] structure i32 { }
[primitive("std::primitive::i64")] structure i64 { }
"#;

fn write_project(prefix: &str, target: &str, source: &str) -> (TempDir, PathBuf, PathBuf) {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("临时目录");
    let root = temp_dir.path();
    let project_dir = root.join("app");
    fs::create_dir_all(project_dir.join("source")).expect("建 source 目录");
    fs::write(root.join("legions.von"), r#"{ name: "library-surface", members: ["app"] }"#).expect("写 workspace 清单");
    fs::write(
        project_dir.join("legion.von"),
        format!(r#"{{ name: "app", artifact: "library", build: [{{ target: "{target}" }}] }}"#),
    )
    .expect("写项目清单");
    fs::write(project_dir.join("source").join("main.v"), source).expect("写入口源码");
    fs::write(project_dir.join("source").join("core_primitive.v"), PRIMITIVES).expect("写原始类型声明");
    let output_dir = project_dir.join("dist").join(target);
    (temp_dir, project_dir, output_dir)
}

fn build(project_dir: &PathBuf, target: &str, output_dir: &PathBuf) -> Result<ExitCode, String> {
    run(&BuildArgs {
        project_dir: project_dir.clone(),
        target: CanonicalTarget::parse(target).expect("target"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    })
    .map_err(|error| error.to_string())
}

fn two_exports_source() -> &'static str {
    r#"namespace app;

[export(name: "first")]
micro first(left: i32, right: i32) -> i32 {
    return left
}

[export(name: "second")]
micro second(left: i32, right: i32) -> i32 {
    return right
}
"#
}

fn call_wasm_both(glue_path: &PathBuf) -> String {
    let runner_path = glue_path.parent().expect("产物目录").join("library_surface_runner.mjs");
    fs::write(
        &runner_path,
        r#"import { pathToFileURL } from "node:url";
const glue = await import(pathToFileURL(process.argv[2]).href);
const out = [];
for (const [name, left, right] of [["first", 2, 3], ["second", 9, 4]]) {
  const value = await glue.callExport(name, left, right);
  out.push(`${name}=${typeof value === "bigint" ? value.toString() : JSON.stringify(value)}`);
}
process.stdout.write(out.join("\n"));
"#,
    )
    .expect("写 Node runner");
    let output = std::process::Command::new("node")
        .arg(&runner_path)
        .arg(glue_path)
        .output()
        .expect("Node 必须可用");
    assert!(
        output.status.success(),
        "node callExport 失败 status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn wasm_library_exports_are_callable_by_name() {
    let (_temp_dir, project_dir, output_dir) = write_project("library-surface-wasm", "wasm", two_exports_source());
    assert_eq!(build(&project_dir, "wasm", &output_dir).expect("构建必须执行"), ExitCode::SUCCESS);

    let manifest = ExecutionManifest::read_from_output_dir(&output_dir)
        .expect("读取 run-contracts")
        .expect("library 构建必须写入 execution manifest");
    assert_eq!(format!("{:?}", manifest.artifact_kind), "Library", "artifact_kind 必须为 library");
    assert!(manifest.run_contracts.is_empty(), "wasm library 不应产生可执行 run contract: {:?}", manifest.run_contracts);
    let names = manifest.library_exports.iter().map(|entry| entry.logical_name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, vec!["first", "second"], "库导出索引必须按导出名列出两个 `[export]`");
    assert!(
        manifest.library_exports.iter().all(|entry| !entry.abi_digest.is_empty()),
        "wasm 库导出必须携带 ABI digest"
    );

    let glue_path = fs::read_dir(&output_dir)
        .expect("读取产物目录")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mjs"))
        .expect("产物目录缺少 `.mjs` Node glue");
    let calls = call_wasm_both(&glue_path);
    assert!(calls.contains("first=2"), "callExport first 必须返回 left=2: {calls}");
    assert!(calls.contains("second=4"), "callExport second 必须返回 right=4: {calls}");
}

#[test]
fn ovm_library_exports_are_callable_by_entry() {
    let (_temp_dir, project_dir, output_dir) = write_project("library-surface-ovm", "ovm", two_exports_source());
    assert_eq!(build(&project_dir, "ovm", &output_dir).expect("构建必须执行"), ExitCode::SUCCESS);

    let manifest = ExecutionManifest::read_from_output_dir(&output_dir)
        .expect("读取 run-contracts")
        .expect("library 构建必须写入 execution manifest");
    assert_eq!(format!("{:?}", manifest.artifact_kind), "Library", "artifact_kind 必须为 library");
    let logical = manifest.run_contracts.iter().map(|contract| contract.logical_entry.as_str()).collect::<Vec<_>>();
    assert_eq!(logical, vec!["first", "second"], "OVM library 必须为每个导出产出 run contract");
    assert_eq!(manifest.library_exports.len(), 2, "库导出索引必须包含两个导出");
    assert!(
        manifest.library_exports.iter().all(|entry| entry.invocation.contains("--entry")),
        "OVM 导出调用方式必须记录 --entry"
    );

    // 产物描述符本身必须是 `ArtifactKind::Library`，不得只靠 manifest 旁路表达库语义。
    let workspace = WorkspaceResolver::discover_for_project(&project_dir).expect("发现 workspace");
    let plan = workspace
        .build_plan(&BuildRequest {
            project_dir: project_dir.clone(),
            target: CanonicalTarget::parse("ovm").expect("target"),
            output_dir: Some(output_dir.clone()),
            ..Default::default()
        })
        .expect("构建计划");
    let report = compile_plan(&plan, false).expect("复用缓存编译");
    assert!(
        report.artifacts.artifacts.iter().any(|artifact| artifact.kind == ArtifactKind::Library),
        "OVM 库产物必须标记 ArtifactKind::Library: {:?}",
        report.artifacts.artifacts
    );

    let artifact = fs::read_dir(&output_dir)
        .expect("读取产物目录")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("odin"))
        .expect("产物目录缺少 `.odin`");
    let bytes = fs::read(&artifact).expect("读取 `.odin`");

    // sidecar 必须按整份 `.odin` 哈希校验；对不上 fail-closed，且合同 digest 与
    // manifest 的库导出 ABI digest 同源。
    let artifact_name = artifact.file_name().expect("文件名").to_string_lossy().to_string();
    let sidecar_path = output_dir.join(nyar_emitter::ovm_abi_sidecar_file(&artifact_name));
    let sidecar: nyar_emitter::OvmAbiSidecar =
        serde_json::from_slice(&fs::read(&sidecar_path).expect("缺按产物命名的 `.ovm-abi.json` sidecar")).expect("sidecar JSON");
    assert_eq!(sidecar.artifact, artifact_name);
    sidecar.verify_against(&bytes).expect("sidecar 必须与 `.odin` 整体哈希一致");
    assert_eq!(
        sidecar.contract.digest, manifest.library_exports[0].abi_digest,
        "sidecar 合同 digest 必须与 manifest 库导出 ABI digest 同源"
    );
    let sidecar_names = sidecar.contract.exports.iter().map(|export| export.logical_name.as_str()).collect::<Vec<_>>();
    assert_eq!(sidecar_names, vec!["first", "second"], "sidecar 合同必须覆盖两个导出");
    let mut tampered = bytes.clone();
    tampered.push(0);
    assert!(
        sidecar.verify_against(&tampered).is_err(),
        "产物字节被改动后 sidecar 校验必须失败闭合"
    );

    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("`.odin` 必须可被 odin-vm 加载");
    let symbol_for = |name: &str| {
        loaded
            .exports
            .iter()
            .map(|export| export.symbol_name.clone())
            .find(|symbol| symbol == name || symbol.ends_with(&format!("::{name}")) || symbol.ends_with(name))
            .unwrap_or_else(|| panic!("产物缺少导出 `{name}`：{:?}", loaded.exports))
    };
    assert_eq!(vm.run(&loaded, &symbol_for("first"), vec![Value::I32(2), Value::I32(3)]).expect("first 执行"), Value::I32(2));
    assert_eq!(vm.run(&loaded, &symbol_for("second"), vec![Value::I32(9), Value::I32(4)]).expect("second 执行"), Value::I32(4));
}

#[test]
fn library_without_exports_fails_on_all_lanes() {
    for (target, prefix) in [("wasm", "library-empty-wasm"), ("ovm", "library-empty-ovm"), ("jvm", "library-empty-jvm"), ("clr", "library-empty-clr")] {
        let (_temp_dir, project_dir, output_dir) =
            write_project(prefix, target, "namespace app;\n\nmicro noop() -> i32 {\n    return 0\n}\n");
        let error = build(&project_dir, target, &output_dir).expect_err("无 `[export]` 的 library 必须结构化失败");
        assert!(error.contains("export"), "`{target}` 失败必须说明缺少导出面: {error}");
        let manifest = ExecutionManifest::read_from_output_dir(&output_dir).expect("读取 manifest");
        assert!(manifest.is_none(), "`{target}` 失败后不得产生 run contract: {manifest:?}");
    }
}
