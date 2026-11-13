//! OVM 闭包载体 P2 端到端：`legion build` 真构建 `.odin`，再交 odin-vm 真跑。
//!
//! 验收面：无捕获 lambda 行为与 P1 之前一致；std `HashSet<i32>::from_list` 的内部
//! 回调（捕获 `result`，只读绑定、经 receiver 修改对象）经 `iter(self, f)` 走
//! `CallIndirect` 后得到去重后的元素数。

use std::{
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

use odin_vm::{OdinVm, Value};
use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::build::{BuildArgs, run},
};
use tempfile::TempDir;

struct Fixture {
    _temp_dir: TempDir,
    project_dir: PathBuf,
}

/// 写夹具项目；`primitives` 为 `None` 时由依赖包提供原始类型声明。
fn create_project(prefix: &str, manifest: &str, source: &str, primitives: Option<&str>) -> Fixture {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("临时目录");
    let project_dir = temp_dir.path().join("app");
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).expect("建 source 目录");
    fs::write(
        temp_dir.path().join("legions.von"),
        r#"{ name: "runtime-smoke", members: ["app"] }"#,
    )
    .expect("写 workspace 清单");
    fs::write(project_dir.join("legion.von"), manifest).expect("写项目清单");
    fs::write(source_dir.join("main.v"), source).expect("写入口源码");
    if let Some(declarations) = primitives {
        fs::write(source_dir.join("core_primitive.v"), declarations).expect("写原始类型声明");
    }
    Fixture { _temp_dir: temp_dir, project_dir }
}

/// 以 `ovm` 为目标构建夹具项目，返回仍持有临时目录的夹具与 `.odin` 产物。
fn build_ovm(prefix: &str, manifest: &str, source: &str, primitives: Option<&str>) -> (Fixture, PathBuf) {
    let fixture = create_project(prefix, manifest, source, primitives);
    let output_dir = fixture.project_dir.join("dist").join("ovm");
    let status = run(&BuildArgs {
        project_dir: fixture.project_dir.clone(),
        target: CanonicalTarget::parse("ovm").expect("ovm target"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    })
    .expect("legion build 必须执行");
    assert_eq!(status, ExitCode::SUCCESS, "OVM 构建必须成功，产物目录 {}", output_dir.display());
    let artifact = fs::read_dir(&output_dir)
        .expect("读取产物目录")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("odin"))
        .unwrap_or_else(|| panic!("产物目录缺少 `.odin`：{}", output_dir.display()));
    (fixture, artifact)
}

/// 把 `.odin` 交 odin-vm 加载并执行 `main`。
fn run_main(artifact: &Path) -> Value {
    let bytes = fs::read(artifact).expect("读取 `.odin` 产物");
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("`.odin` 产物必须可被 odin-vm 加载");
    let entry = loaded
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|name| name.ends_with("main"))
        .unwrap_or_else(|| panic!("产物必须导出 main：{:?}", loaded.exports));
    vm.run(&loaded, &entry, Vec::new()).expect("main 必须在 odin-vm 运行时执行成功")
}

/// 无捕获 lambda 仍按闭包对象编码（code 槽 + null 环境），间接调用结果不变。
#[test]
fn ovm_named_lambda_without_capture_executes_unchanged() {
    let (_fixture, artifact) = build_ovm(
        "p2-ovm-nocapture",
        r#"{
            name: "app",
            build: [{ target: "ovm" }]
        }"#,
        r#"namespace app;

micro apply(f: micro(i64) -> i64) -> i64 {
    return f(41)
}

[main]
micro main() -> i64 {
    return apply(micro(value: i64) -> i64 {
        return value
    })
}
"#,
        Some(r#"namespace std.primitive;
[primitive("std::primitive::i64")] structure i64 { }"#),
    );
    assert_eq!(run_main(&artifact), Value::I64(41), "无捕获 lambda 间接调用必须返回 41");
}

/// std 与 OVM SDK adaptor 的 `std: workspace` 依赖需要同仓成员身份：
/// 在临时目录复刻最小 workspace，只拷贝 manifest 与 `source/`，不改动 valkyrie.v。
fn create_std_workspace(std_root: &Path) -> (TempDir, PathBuf) {
    let temp_dir = tempfile::Builder::new().prefix("p2-ovm-hashset-capture").tempdir().expect("临时目录");
    let root = temp_dir.path();
    for (name, source_project) in [
        ("std", std_root.join("projects").join("std")),
        ("std.adaptor.ovm", std_root.join("projects").join("std.adaptors._").join("std.adaptor.ovm")),
        ("unicode.types", std_root.join("projects").join("unicode._").join("unicode.types")),
        ("unicode.case", std_root.join("projects").join("unicode._").join("unicode.case")),
    ] {
        let target = root.join(name);
        fs::create_dir_all(&target).expect("建依赖目录");
        fs::copy(source_project.join("legion.von"), target.join("legion.von")).expect("拷贝依赖清单");
        copy_source_tree(&source_project.join("source"), &target.join("source"));
    }
    fs::write(
        root.join("legions.von"),
        r#"{ name: "p2-closure", members: ["std", "std.adaptor.ovm", "unicode.types", "unicode.case", "app"] }"#,
    )
    .expect("写 workspace 清单");
    let project_dir = root.join("app");
    fs::create_dir_all(project_dir.join("source")).expect("建入口 source 目录");
    fs::write(
        project_dir.join("legion.von"),
        r#"{
            name: "app",
            dependencies: { "std": "workspace", "std.adaptor.ovm": "workspace" },
            build: [{ target: "ovm" }]
        }"#,
    )
    .expect("写项目清单");
    (temp_dir, project_dir)
}

fn copy_source_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("建源码目录");
    for entry in fs::read_dir(from).expect("读取源码目录") {
        let entry = entry.expect("源码条目");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("条目类型").is_dir() {
            copy_source_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("拷贝源码文件");
        }
    }
}

/// std `HashSet<i32>::from_list` 回调路径真构建 + 真跑。
#[test]
fn ovm_hashset_from_list_callback_with_capture_executes() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let (temp_dir, project_dir) = create_std_workspace(&std_root);
    fs::write(
        project_dir.join("source").join("main.v"),
        r#"namespace app;

using std.collection::{ArrayList, HashSet};

[main]
micro main() -> i32 {
    let mut values: ArrayList<i32> = ArrayList::new(4)
    values.push(1)
    values.push(2)
    values.push(2)
    values.push(3)
    let set: HashSet<i32> = HashSet::from_list(values)
    let mut seen: ArrayList<i32> = ArrayList::new(4)
    set.iter(micro(value: i32) -> unit {
        seen.push(value)
    })
    return seen.length as i32
}
"#,
    )
    .expect("写入口源码");

    let output_dir = project_dir.join("dist").join("ovm");
    let status = run(&BuildArgs {
        project_dir: project_dir.clone(),
        target: CanonicalTarget::parse("ovm").expect("ovm target"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    })
    .expect("legion build 必须执行");
    assert_eq!(status, ExitCode::SUCCESS, "std 依赖的 OVM 构建必须成功，产物目录 {}", output_dir.display());
    let artifact = fs::read_dir(&output_dir)
        .expect("读取产物目录")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("odin"))
        .unwrap_or_else(|| panic!("产物目录缺少 `.odin`：{}", output_dir.display()));
    let value = run_main(&artifact);
    drop(temp_dir);
    assert_eq!(value, Value::I32(3), "捕获回调必须把去重后的 3 个元素写入 seen");
}

fn valkyrie_v_std_root() -> Option<PathBuf> {
    if let Ok(root) = std::env::var("VALKYRIE_V_ROOT") {
        let path = PathBuf::from(root);
        if path.join("projects").join("std").join("legion.von").is_file() {
            return Some(path);
        }
    }
    let sibling = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../../valkyrie.v");
    if sibling.join("projects").join("std").join("legion.von").is_file() {
        return Some(sibling);
    }
    None
}

/// 旧 `.nyar` 执行产物存在时必须结构化拒绝，不得静默覆盖或双读。
#[test]
fn ovm_build_refuses_to_overwrite_legacy_nyar_artifact() {
    let fixture = create_project(
        "ovm-legacy-output",
        r#"{
            name: "app",
            build: [{ target: "ovm" }]
        }"#,
        r#"namespace app;

[main]
micro main() -> i32 {
    return 0
}
"#,
        Some(r#"namespace std.primitive;
[primitive("std::primitive::i32")] structure i32 { }"#),
    );
    let output_dir = fixture.project_dir.join("dist").join("ovm");
    fs::create_dir_all(&output_dir).expect("建产物目录");
    fs::write(output_dir.join("stale.nyar"), b"\x4E\x59\x41\x52").expect("写旧执行产物");

    let error = run(&BuildArgs {
        project_dir: fixture.project_dir.clone(),
        target: CanonicalTarget::parse("ovm").expect("ovm target"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    })
    .expect_err("旧 `.nyar` 执行产物必须被拒绝");
    let message = error.to_string();
    assert!(message.contains(".nyar"), "拒绝必须指出旧产物身份: {message}");
    assert!(message.contains(".odin"), "拒绝必须指出目标产物身份: {message}");
    assert!(!output_dir.join("app.odin").is_file(), "拒绝后不得写出 `.odin` 产物");
}
