//! WASM 闭包载体 P4 端到端：`legion build` 真构建 wasm 产物，再交 Node + JS glue 真跑。
//!
//! 验收面（合同 §5.3）：
//! - 函数值 = gc struct `[code: i32 table 槽, env: anyref]`，无捕获路径 env 为 null；
//! - `call_indirect` 的 functype 恒为「可见形参 + 一个尾部 anyref 环境槽」；
//! - std `HashSet<i32>::from_list` 的 `iter` 回调（捕获 `seen`）经间接调用得到 3；
//! - 无捕获 lambda 探针行为不变（41）；
//! - `sorted_by` 的比较器经形参转发后再间接调用，环境槽跨函数边界仍有效。

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::build::{BuildArgs, run},
};
use tempfile::TempDir;

struct WasmArtifact {
    _temp_dir: TempDir,
    output_dir: PathBuf,
    glue_path: PathBuf,
}

/// 无 std 的最小 wasm 夹具：`artifact: "library"` + `[export]`，Node glue 用 `callExport`。
fn build_standalone_wasm(prefix: &str, source: &str) -> WasmArtifact {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("临时目录");
    let root = temp_dir.path();
    let project_dir = root.join("app");
    fs::create_dir_all(project_dir.join("source")).expect("建 source 目录");
    fs::write(root.join("legions.von"), r#"{ name: "p4-wasm", members: ["app"] }"#).expect("写 workspace 清单");
    fs::write(
        project_dir.join("legion.von"),
        r#"{ name: "app", artifact: "library", build: [{ target: "wasm" }] }"#,
    )
    .expect("写项目清单");
    fs::write(project_dir.join("source").join("main.v"), source).expect("写入口源码");
    fs::write(
        project_dir.join("source").join("core_primitive.v"),
        r#"[primitive("std::primitive::i32")] structure i32 { }
[primitive("std::primitive::i64")] structure i64 { }"#,
    )
    .expect("写原始类型声明");
    build_and_find_glue(temp_dir, project_dir)
}

/// std 与 wasm SDK adaptor 需要同仓成员身份：在临时目录复刻最小 workspace，
/// 只拷贝 manifest 与 `source/`，不改动 valkyrie.v。
fn build_std_wasm(prefix: &str, std_root: &Path, source: &str) -> WasmArtifact {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("临时目录");
    let root = temp_dir.path();
    for (name, source_project) in [
        ("std", std_root.join("projects").join("std")),
        ("std.adaptor.wasm", std_root.join("projects").join("std.adaptors._").join("std.adaptor.wasm")),
    ] {
        let target = root.join(name);
        fs::create_dir_all(&target).expect("建依赖目录");
        fs::copy(source_project.join("legion.von"), target.join("legion.von")).expect("拷贝依赖清单");
        copy_source_tree(&source_project.join("source"), &target.join("source"));
    }
    fs::write(
        root.join("legions.von"),
        r#"{ name: "p4-wasm-std", members: ["std", "std.adaptor.wasm", "app"] }"#,
    )
    .expect("写 workspace 清单");
    let project_dir = root.join("app");
    fs::create_dir_all(project_dir.join("source")).expect("建入口 source 目录");
    fs::write(
        project_dir.join("legion.von"),
        r#"{
            name: "app",
            artifact: "library",
            dependencies: { "std": "workspace", "std.adaptor.wasm": "workspace" },
            build: [{ target: "wasm" }]
        }"#,
    )
    .expect("写项目清单");
    fs::write(project_dir.join("source").join("main.v"), source).expect("写入口源码");
    build_and_find_glue(temp_dir, project_dir)
}

fn build_and_find_glue(temp_dir: TempDir, project_dir: PathBuf) -> WasmArtifact {
    let output_dir = project_dir.join("dist").join("wasm");
    let status = run(&BuildArgs {
        project_dir: project_dir.clone(),
        target: CanonicalTarget::parse("wasm").expect("wasm target"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    })
    .expect("legion build 必须执行");
    assert_eq!(status, ExitCode::SUCCESS, "WASM 构建必须成功，产物目录 {}", output_dir.display());
    let glue_path = fs::read_dir(&output_dir)
        .expect("读取产物目录")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mjs"))
        .unwrap_or_else(|| panic!("产物目录缺少 `.mjs` Node glue：{}", output_dir.display()));
    WasmArtifact { _temp_dir: temp_dir, output_dir, glue_path }
}

fn copy_source_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("建源码目录");
    for entry in fs::read_dir(from).expect("读取源码目录") {
        let entry = entry.expect("源码条目");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("条目类型").is_dir() {
            copy_source_tree(&entry.path(), &target);
        }
        else {
            fs::copy(entry.path(), target).expect("拷贝源码文件");
        }
    }
}

/// Node 经 JS glue 调用导出函数（标量 i32 返回值按 JSON 读回）。
fn call_export(artifact: &WasmArtifact, export_name: &str) -> String {
    let runner_path = artifact.output_dir.join("p4_wasm_runner.mjs");
    fs::write(
        &runner_path,
        r#"import { pathToFileURL } from "node:url";
const glue = await import(pathToFileURL(process.argv[2]).href);
const value = await glue.callExport(process.argv[3]);
process.stdout.write(typeof value === "bigint" ? value.toString() : JSON.stringify(value ?? null));
"#,
    )
    .expect("写入 Node runner");
    let output = Command::new("node")
        .arg(&runner_path)
        .arg(&artifact.glue_path)
        .arg(export_name)
        .output()
        .expect("Node 必须可用（主动交付线宿主）");
    assert!(
        output.status.success(),
        "node callExport 失败 status={:?}\nstderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// 无捕获 lambda 探针：闭包对象照旧构造（code 槽 + null 环境），间接调用结果不变。
#[test]
fn wasm_named_lambda_without_capture_executes_unchanged() {
    let artifact = build_standalone_wasm(
        "p4-wasm-nocapture",
        r#"namespace app;

micro apply(f: micro(i32) -> i32) -> i32 {
    return f(41)
}

[export(name: "run")]
micro run() -> i32 {
    return apply(micro(value: i32) -> i32 {
        return value
    })
}
"#,
    );
    assert_eq!(call_export(&artifact, "run"), "41", "无捕获 lambda 间接调用必须返回 41");
}

/// std `HashSet<i32>::from_list` + `iter(self, f)` 回调路径：真构建 + Node 真跑。
#[test]
fn wasm_hashset_from_list_callback_with_capture_executes() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-hashset-capture",
        &std_root,
        r#"namespace app;

using std.collection::{ArrayList, HashSet};

[export(name: "run")]
micro run() -> i32 {
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
    );
    assert_eq!(call_export(&artifact, "run"), "3", "捕获回调必须把去重后的 3 个元素写入 seen");
}

/// `sorted_by` + 闭包比较器：比较器经 `sorted_by` → `merge_sort` → `insertion_sort`
/// 形参转发后再间接调用，验证 env 槽跨函数边界仍随闭包对象流动。
#[test]
fn wasm_sorted_by_with_closure_comparator_executes() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-sorted-by",
        &std_root,
        r#"namespace app;

using std.collection::{ArrayList, sorted_by};

[export(name: "run")]
micro run() -> i32 {
    let mut items: ArrayList<i32> = ArrayList::new(3)
    items.push(1)
    items.push(3)
    items.push(2)
    let direction: i32 = -1
    let sorted: ArrayList<i32> = sorted_by(items, micro(a: i32, b: i32) -> i32 {
        return (a - b) * direction
    })
    return sorted.get(1).unwrap()
}
"#,
    );
    assert_eq!(call_export(&artifact, "run"), "3", "捕获 direction 的降序比较器必须把最大值排到首位");
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
