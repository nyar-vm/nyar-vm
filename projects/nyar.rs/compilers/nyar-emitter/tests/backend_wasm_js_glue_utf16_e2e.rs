//! JS glue 的 `utf16_*` 宿主函数往返：证明 UTF-16 code unit 语义没有被 utf8
//! scalar 语义吞掉（`"😀".length === 2`、按 code unit 切片、lone surrogate 原样保留）。
//!
//! 这里直接执行启动壳里生成的 `importObject.env` 函数：句柄经 `hostIntern` 建表、
//! 经 `hostPathFromRef` 还原，覆盖“建表 → 还原 → 再建表”的真实往返。

use std::process::Command;

fn run_probe() -> String {
    let dir = tempfile::tempdir().expect("tempdir");
    // 启动壳顶层只 `readFileSync` 读字节，不实例化；空文件即可。
    std::fs::write(dir.path().join("utf16_probe.wasm"), []).expect("write wasm placeholder");
    let imports: Vec<(String, String)> = [
        ("env", "utf8_length"),
        ("env", "utf8_equals"),
        ("env", "utf16_length"),
        ("env", "utf16_slice"),
        ("env", "utf16_equals"),
        ("env", "utf16_concat"),
        ("env", "const_utf16"),
        ("env", "utf8_to_utf16"),
        ("env", "utf16_to_utf8"),
    ]
    .into_iter()
    .map(|(module, field)| (module.to_string(), field.to_string()))
    .collect();
    let launcher = nyar_emitter::build_node_launcher("utf16_probe", &imports, &["probe".to_string()], true, None);
    // 测试专属入口：启动壳本身导出 `callExport`，这里只额外暴露 `importObject`。
    std::fs::write(dir.path().join("utf16_probe.mjs"), format!("{launcher}\nexport {{ importObject }};\n")).expect("write launcher");
    let script = r#"
import { importObject } from "./utf16_probe.mjs";
const env = importObject.env;
const checks = [];
function eq(name, actual, expected) {
    if (actual !== expected) throw new Error(`${name}: expected ${expected}, got ${actual}`);
    checks.push(name);
}
// code unit ≠ scalar：emoji 是 1 个 scalar、2 个 code unit。
eq("utf16_length(emoji) counts code units", env.utf16_length("😀"), 2);
eq("utf8_length(emoji) counts scalars", env.utf8_length("😀"), 1);
// 按 code unit 切片会留下一个高位代理；utf8 scalar 切片会留下整个 emoji。
const high = env.utf16_slice("😀", 0, 1);
eq("code-unit slice yields high surrogate", env.utf16_equals(high, "\uD83D"), 1);
eq("high surrogate is one code unit", env.utf16_length(high), 1);
// lone surrogate 必须原样保留；utf8 编码路径会把它替换成 U+FFFD。
const lone = env.utf16_slice("a\uD800b", 1, 1);
eq("lone surrogate survives", env.utf16_equals(lone, "\uD800"), 1);
eq("lone surrogate length", env.utf16_length(lone), 1);
eq("const_utf16 interns literal", env.utf16_equals(env.const_utf16(0), "probe"), 1);
eq("utf8_to_utf16 keeps text", env.utf16_equals(env.utf8_to_utf16("中文😀"), "中文😀"), 1);
eq("utf16_to_utf8 keeps text", env.utf8_equals(env.utf16_to_utf8("中文😀"), "中文😀"), 1);
eq("utf16_concat keeps code units", env.utf16_length(env.utf16_concat("😀", "😀")), 4);
process.stdout.write(`ok ${checks.length}`);
"#;
    let script_path = dir.path().join("run.mjs");
    std::fs::write(&script_path, script).expect("write driver");
    let output = Command::new("node").arg(&script_path).current_dir(dir.path()).output().expect("spawn node");
    if !output.status.success() {
        panic!(
            "node utf16 probe failed status={:?}\nstdout={}\nstderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn utf16_glue_roundtrip_preserves_code_unit_semantics() {
    assert_eq!(
        run_probe(),
        "ok 10",
        "utf16 宿主函数必须按 UTF-16 code unit 语义往返，而不是 utf8 scalar 语义"
    );
}
