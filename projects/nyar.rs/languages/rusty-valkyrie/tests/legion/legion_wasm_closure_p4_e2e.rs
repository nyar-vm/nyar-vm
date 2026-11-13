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
        r#"namespace std.primitive;
[primitive("std::primitive::i32")] structure i32 { }
[primitive("std::primitive::i64")] structure i64 { }"#,
    )
    .expect("写原始类型声明");
    build_and_find_glue(temp_dir, project_dir)
}

/// std 与 wasm SDK adaptor 需要同仓成员身份：在临时目录复刻最小 workspace，
/// 只拷贝 manifest 与 `source/`，不改动 valkyrie.v。
fn build_std_wasm(prefix: &str, std_root: &Path, source: &str) -> WasmArtifact {
    let (temp_dir, project_dir) = write_std_workspace(prefix, std_root, source);
    build_and_find_glue(temp_dir, project_dir)
}

/// 与 [`build_std_wasm`] 同一 std 工作区，但要求构建失败并返回结构化诊断文本。
fn build_std_wasm_error(prefix: &str, std_root: &Path, source: &str) -> String {
    let (temp_dir, project_dir) = write_std_workspace(prefix, std_root, source);
    let output_dir = project_dir.join("dist").join("wasm");
    let error = run(&BuildArgs {
        project_dir,
        target: CanonicalTarget::parse("wasm").expect("wasm target"),
        output_dir: Some(output_dir),
        debug_artifacts: false,
    })
    .expect_err("该源码必须被编译流水线结构化拒绝");
    let _ = temp_dir;
    error.to_string()
}

fn write_std_workspace(prefix: &str, std_root: &Path, source: &str) -> (TempDir, PathBuf) {
    let temp_dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("临时目录");
    let root = temp_dir.path();
    for (name, source_project) in [
        ("std", std_root.join("projects").join("std")),
        ("std.adaptor.wasm", std_root.join("projects").join("std.adaptors._").join("std.adaptor.wasm")),
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
        r#"{ name: "p4-wasm-std", members: ["std", "std.adaptor.wasm", "unicode.types", "unicode.case", "app"] }"#,
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
    (temp_dir, project_dir)
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
using std.operator::{Ordering, Less, Equal, Greater};

[export(name: "run")]
micro run() -> i32 {
    let mut items: ArrayList<i32> = ArrayList::new(3)
    items.push(1)
    items.push(3)
    items.push(2)
    let descending: bool = true
    let sorted: ArrayList<i32> = sorted_by(items, micro(a: i32, b: i32) -> Ordering {
        if descending {
            if a < b { return Greater }
            if b < a { return Less }
            return Equal
        }
        if a < b { return Less }
        if b < a { return Greater }
        return Equal
    })
    return sorted.get(1).unwrap()
}
"#,
    );
    assert_eq!(call_export(&artifact, "run"), "3", "捕获 direction 的降序比较器必须把最大值排到首位");
}

/// 库导出返回 `utf8`（宿主字符串）：真构建 + Node 真跑，断言物化后的真实字符串值，
/// 而不是断言导出符号或签名形态。修复前返回值未物化时宿主读到的是 anyref 槽。
#[test]
fn wasm_library_export_utf8_return_executes_with_real_value() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-utf8-return",
        &std_root,
        r#"namespace app;

[export(name: "hello")]
micro hello() -> utf8 {
    let mut builder: Utf8Builder = Utf8Builder::new(4)
    builder.append(char::from(104))
    builder.append(char::from(105))
    return builder.build()
}
"#,
    );
    assert_eq!(call_export(&artifact, "hello"), "\"hi\"", "utf8 返回值必须物化为宿主可读的真实字符串");
}

/// `HashMap<utf8, i64>` 经导出边界进出：键的宿主 i32 句柄载体在容器内部到
/// managed 字符串槽的单一边界点完成物化，真跑断言读回的整数值。
#[test]
fn wasm_hashmap_utf8_key_boundary_roundtrips_real_value() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-utf8-hashmap",
        &std_root,
        r#"namespace app;

using std.collection::HashMap;

[export(name: "roundtrip")]
micro roundtrip(key: utf8, value: i64) -> i64 {
    let mut map: HashMap<utf8, i64> = HashMap::new(4)
    map.insert(key, value)
    return map.get(key).unwrap()
}
"#,
    );
    assert_eq!(call_export_with_args(&artifact, "roundtrip", r#"["k", "7"]"#), "7", "HashMap<utf8, _> 的键边界必须读写一致");
}

/// Node 经 JS glue 带实参调用导出函数（字符串实参按 UTF-8 句柄编组）。
fn call_export_with_args(artifact: &WasmArtifact, export_name: &str, args_json: &str) -> String {
    let runner_path = artifact.output_dir.join("p4_wasm_runner_args.mjs");
    fs::write(
        &runner_path,
        r#"import { pathToFileURL } from "node:url";
const glue = await import(pathToFileURL(process.argv[2]).href);
const args = JSON.parse(process.argv[4] ?? "[]");
const value = await glue.callExport(process.argv[3], ...args);
process.stdout.write(typeof value === "bigint" ? value.toString() : JSON.stringify(value ?? null));
"#,
    )
    .expect("写入 Node runner");
    let output = Command::new("node")
        .arg(&runner_path)
        .arg(&artifact.glue_path)
        .arg(export_name)
        .arg(args_json)
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

/// `utf8` 别名/载体：字符串字面量进入 `utf8` 局部槽与形参槽、以及 `[utf8]` 返回，
/// 全部真构建 + Node 真跑断言值（而不是只断言签名）。
#[test]
fn wasm_utf8_alias_literals_execute_with_real_values() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-utf8-alias",
        &std_root,
        r#"namespace app;

[export(name: "let_literal")]
micro let_literal() -> i64 {
    let text: utf8 = "abc"
    return text.length() as i64
}

micro text_length(text: utf8) -> i64 {
    return text.length() as i64
}

[export(name: "call_literal")]
micro call_literal() -> i64 {
    return text_length("abc")
}

[export(name: "words")]
micro words() -> [utf8] {
    return ["a", "bb", "ccc"]
}
"#,
    );
    assert_eq!(call_export(&artifact, "let_literal"), "3", "`let text: utf8 = \"abc\"` 必须真跑得到长度 3");
    assert_eq!(call_export(&artifact, "call_literal"), "3", "字面量传 `utf8` 形参必须真跑得到长度 3");
    assert_eq!(
        call_export(&artifact, "words"),
        r#"["a","bb","ccc"]"#,
        "`[utf8]` 返回必须物化为宿主可读的真实字符串数组"
    );
}

/// 文本编码兼容只覆盖同编码载体：整数不能落进 `utf8` 槽，且必须是结构化诊断而非 panic。
#[test]
fn wasm_utf8_alias_rejects_incompatible_initializer_with_structured_diagnostic() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let error = build_std_wasm_error(
        "p4-wasm-utf8-reject",
        &std_root,
        r#"namespace app;

[export(name: "bad")]
micro bad() -> i64 {
    let text: utf8 = 42
    return 0
}
"#,
    );
    assert!(error.contains("incompatible with the declared type"), "必须是类型不兼容的结构化诊断: {error}");
    assert!(error.contains("`let` initializer"), "诊断必须指明 `let` 初始化位置: {error}");
}

/// `[T]` 的 `for-in`：固定数组经 std 的 `IntoIterator` 实现迭代，真跑断言求和值。
#[test]
fn wasm_array_forin_iterates_with_real_values() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-array-forin",
        &std_root,
        r#"namespace app;

[export(name: "sum")]
micro sum(nums: [i64]) -> i64 {
    let mut total: i64 = 0
    loop value in nums {
        total = total + value
    }
    return total
}
"#,
    );
    assert_eq!(
        call_export_with_args(&artifact, "sum", r#"[["10", "20", "30"]]"#),
        "60",
        "`[i64]` 的 for-in 必须逐元素求和"
    );
}

/// `utf8` 的 `for-in`：按码点（Unicode scalar）迭代，真跑断言元素个数。
#[test]
fn wasm_utf8_forin_iterates_with_real_values() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-utf8-forin",
        &std_root,
        r#"namespace app;

[export(name: "count")]
micro count() -> i64 {
    let text: utf8 = "héllo"
    let mut total: i64 = 0
    loop glyph in text {
        total = total + 1
    }
    return total
}
"#,
    );
    assert_eq!(call_export(&artifact, "count"), "5", "`utf8` 的 for-in 必须按码点计数");
}

/// 非可迭代值被结构化拒绝：诊断必须说明缺 `IntoIterator`，不能是 panic。
#[test]
fn wasm_forin_rejects_non_iterable_with_structured_diagnostic() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let error = build_std_wasm_error(
        "p4-wasm-forin-reject",
        &std_root,
        r#"namespace app;

[export(name: "bad")]
micro bad() -> i64 {
    let mut total: i64 = 0
    loop value in 42 {
        total = total + value
    }
    return total
}
"#,
    );
    assert!(error.contains("IntoIterator"), "必须是缺 `IntoIterator` 的结构化诊断: {error}");
}

/// wasm 宿主 `trim` 必须与 std 的 Unicode `White_Space` 合同等价：删 U+0085，
/// 不删 U+FEFF 与 U+200B。`String.prototype.trim` 的两个码点行为正好相反。
#[test]
fn wasm_utf8_trim_matches_std_white_space_contract() {
    let Some(std_root) = valkyrie_v_std_root()
    else {
        eprintln!("skip: valkyrie.v not found (set VALKYRIE_V_ROOT or clone beside nyar-vm.rs)");
        return;
    };
    let artifact = build_std_wasm(
        "p4-wasm-utf8-trim",
        &std_root,
        r#"namespace app;

micro wrap_right(code: u32) -> utf8 {
    let mut builder: Utf8Builder = Utf8Builder::new(4)
    builder.append(char::from(97))
    builder.append(char::from(code))
    return builder.build()
}

micro wrap_both(code: u32) -> utf8 {
    let mut builder: Utf8Builder = Utf8Builder::new(4)
    builder.append(char::from(code))
    builder.append(char::from(97))
    builder.append(char::from(code))
    return builder.build()
}

[export(name: "trim_nel")]
micro trim_nel() -> utf8 {
    return wrap_both(133).trim()
}

[export(name: "trim_bom")]
micro trim_bom() -> utf8 {
    return wrap_right(65279).trim()
}

[export(name: "trim_zwsp")]
micro trim_zwsp() -> utf8 {
    return wrap_right(8203).trim()
}

# 首尾各一个 U+FEFF；长度在 wasm 内计算，避开宿主解码丢前导 BOM 的干扰。
[export(name: "trim_bom_length")]
micro trim_bom_length() -> i64 {
    return wrap_both(65279).trim().length() as i64
}

[export(name: "replace_aaa")]
micro replace_aaa() -> utf8 {
    let text: utf8 = "aaa"
    return text.replace("aa", "b")
}
"#,
    );
    assert_eq!(call_export(&artifact, "trim_nel"), r#""a""#, "U+0085 是 White_Space，必须被删去");
    assert_eq!(
        call_export(&artifact, "trim_bom"),
        format!("\"a{}\"", "\u{feff}"),
        "U+FEFF 不是 White_Space，尾部必须保留"
    );
    assert_eq!(
        call_export(&artifact, "trim_zwsp"),
        format!("\"a{}\"", "\u{200b}"),
        "U+200B 不是 White_Space，尾部必须保留"
    );
    assert_eq!(call_export(&artifact, "trim_bom_length"), "3", "首尾 U+FEFF 都不是 White_Space，长度必须仍为 3");
    assert_eq!(call_export(&artifact, "replace_aaa"), r#""ba""#, "非重叠从左到右替换：aaa 的 aa→b 得 ba");
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
