//! Inline smoke project helpers for Legion integration tests (no legion-fixtures corpus).

use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

pub struct SmokeProject {
    _temp_dir: TempDir,
    pub project_dir: PathBuf,
}

/// 源级 `core/primitive` 声明族：数值原始类型名不是语言内建，必须绑定到源码声明身份。
/// 夹具项目声明 `"core": false`，因此在 `source/` 下写入等价声明文件；属性值给出
/// `core::primitive::*` 身份，与内联编译测试的 `PRIMITIVE_I32` 保持同一合同。
const CORE_PRIMITIVE_DECLARATIONS: &str = concat!(
    "[primitive(\"core::primitive::i8\")] structure i8 { }\n",
    "[primitive(\"core::primitive::i16\")] structure i16 { }\n",
    "[primitive(\"core::primitive::i32\")] structure i32 { }\n",
    "[primitive(\"core::primitive::i64\")] structure i64 { }\n",
    "[primitive(\"core::primitive::i128\")] structure i128 { }\n",
    "[primitive(\"core::primitive::u8\")] structure u8 { }\n",
    "[primitive(\"core::primitive::u16\")] structure u16 { }\n",
    "[primitive(\"core::primitive::u32\")] structure u32 { }\n",
    "[primitive(\"core::primitive::u64\")] structure u64 { }\n",
    "[primitive(\"core::primitive::u128\")] structure u128 { }\n",
    "[primitive(\"core::primitive::isize\")] structure isize { }\n",
    "[primitive(\"core::primitive::usize\")] structure usize { }\n",
    "[primitive(\"core::primitive::f32\")] structure f32 { }\n",
    "[primitive(\"core::primitive::f64\")] structure f64 { }\n",
    "[primitive(\"core::primitive::f128\")] structure f128 { }\n",
    "[primitive(\"core::primitive::byte\")] structure byte { }\n",
    "[primitive(\"core::primitive::sbyte\")] structure sbyte { }\n",
    // 运行时夹具的 `main` 以 `ExitCode` 表达进程退出码；该名字来自 `std` 适配层，
    // 夹具项目声明 `\"std\": false`，因此在源级给出等价别名，避免依赖未链接的 std 组。
    "type ExitCode = i32;\n",
    "micro ExitCode(value: i32): i32 { return value }\n",
);

/// 写入夹具入口源码，并同时提供源级 primitive 声明。
fn write_main_source(source_dir: &Path, source: &str) {
    fs::write(source_dir.join("main.v"), source).unwrap();
    fs::write(source_dir.join("core_primitive.v"), CORE_PRIMITIVE_DECLARATIONS).unwrap();
}

pub fn create_smoke_project(prefix: &str) -> SmokeProject {
    create_fixture(
        prefix,
        default_build_manifest(),
        r#"[main]
micro main(): i64 {
    return 0
}
"#,
    )
}

pub fn create_smoke_project_with_source(prefix: &str, source: &str) -> SmokeProject {
    create_fixture(prefix, default_build_manifest(), source)
}

pub fn create_smoke_project_with_build(prefix: &str, build_block: &str, source: &str) -> SmokeProject {
    create_fixture(prefix, build_block, source)
}

pub fn create_smoke_project_with_manifest(prefix: &str, manifest: &str, source: &str) -> SmokeProject {
    create_manifest_fixture(prefix, manifest, source, true)
}

pub fn create_local_package_project_with_manifest(prefix: &str, manifest: &str, source: &str) -> SmokeProject {
    create_manifest_fixture(prefix, manifest, source, false)
}

pub fn create_script_project_with_manifest(prefix: &str, manifest: &str, source: &str) -> SmokeProject {
    create_standalone_manifest_fixture(prefix, manifest, source)
}

pub fn create_nested_workspace_member_project_with_manifest(prefix: &str, manifest: &str, source: &str) -> SmokeProject {
    create_nested_workspace_fixture(prefix, Path::new("subspace").join("projects").join("app"), manifest, source)
}

pub fn create_local_package_project(prefix: &str, build_block: &str, source: &str) -> SmokeProject {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = temp_dir.path();
    let project_dir = root.join("app");
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        root.join("legions.von"),
        r#"{
    name: "runtime-smoke",
    members: []
}
"#,
    )
    .unwrap();
    fs::write(
        project_dir.join("legion.von"),
        format!(
            r#"{{
    name: "app",
    build: [
        {}
    ]
}}
"#,
            build_block
        ),
    )
    .unwrap();
    write_main_source(&source_dir, source);

    SmokeProject { _temp_dir: temp_dir, project_dir: canonicalize_lossy(&project_dir) }
}

fn create_fixture(prefix: &str, build_block: &str, source: &str) -> SmokeProject {
    let manifest = format!(
        r#"{{
    name: "app",
    build: [
        {}
    ]
}}
"#,
        build_block
    );
    create_manifest_fixture(prefix, &manifest, source, true)
}

fn create_manifest_fixture(prefix: &str, manifest: &str, source: &str, include_workspace_member: bool) -> SmokeProject {
    create_nested_manifest_fixture(prefix, PathBuf::from("app"), manifest, source, include_workspace_member)
}

fn create_nested_manifest_fixture(
    prefix: &str,
    project_relative_dir: PathBuf,
    manifest: &str,
    source: &str,
    include_workspace_member: bool,
) -> SmokeProject {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = temp_dir.path();
    let project_dir = root.join(&project_relative_dir);
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let workspace_member = project_relative_dir.to_string_lossy().replace('\\', "/");
    let workspace_manifest = if include_workspace_member {
        format!(
            r#"{{
    name: "runtime-smoke",
    members: [
        "{}"
    ]
}}
"#,
            workspace_member
        )
    }
    else {
        r#"{
    name: "runtime-smoke",
    members: []
}
"#
        .to_string()
    };
    fs::write(root.join("legions.von"), workspace_manifest).unwrap();
    fs::write(project_dir.join("legion.von"), manifest).unwrap();
    write_main_source(&source_dir, source);

    SmokeProject { _temp_dir: temp_dir, project_dir: canonicalize_lossy(&project_dir) }
}

fn create_nested_workspace_fixture(prefix: &str, project_relative_dir: PathBuf, manifest: &str, source: &str) -> SmokeProject {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = temp_dir.path();
    let subspace_dir = root.join("subspace");
    let project_dir = root.join(&project_relative_dir);
    let source_dir = project_dir.join("source");
    let nested_member = project_relative_dir
        .strip_prefix("subspace")
        .unwrap_or(project_relative_dir.as_path())
        .to_string_lossy()
        .trim_start_matches(['\\', '/'])
        .replace('\\', "/");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        root.join("legions.von"),
        r#"{
    name: "runtime-smoke",
    members: [
        "subspace"
    ]
}
"#,
    )
    .unwrap();
    fs::write(
        subspace_dir.join("legions.von"),
        format!(
            r#"{{
    name: "nested-runtime-smoke",
    members: [
        "{nested_member}"
    ]
}}
"#
        ),
    )
    .unwrap();
    fs::write(project_dir.join("legion.von"), manifest).unwrap();
    write_main_source(&source_dir, source);

    SmokeProject { _temp_dir: temp_dir, project_dir: canonicalize_lossy(&project_dir) }
}

fn create_root_manifest_fixture(prefix: &str, manifest: &str, source: &str, include_workspace_member: bool) -> SmokeProject {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    let workspace_manifest = if include_workspace_member {
        r#"{
    name: "runtime-smoke",
    members: [
        "."
    ]
}
"#
    }
    else {
        r#"{
    name: "runtime-smoke",
    members: []
}
"#
    };
    fs::write(root.join("legions.von"), workspace_manifest).unwrap();
    fs::write(root.join("legion.von"), manifest).unwrap();
    write_main_source(&source_dir, source);

    SmokeProject { _temp_dir: temp_dir, project_dir: canonicalize_lossy(&root) }
}

fn create_standalone_manifest_fixture(prefix: &str, manifest: &str, source: &str) -> SmokeProject {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(root.join("legion.von"), manifest).unwrap();
    write_main_source(&source_dir, source);

    SmokeProject { _temp_dir: temp_dir, project_dir: canonicalize_lossy(&root) }
}

fn default_build_manifest() -> &'static str {
    r#"{
            target: "clr",
            msil: true
        }"#
}

/// 规范化路径（失败时保留原路径）。
fn canonicalize_lossy(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
