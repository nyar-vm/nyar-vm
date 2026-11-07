//! `f64` / `f32` 位模式重解释的原始类型合同端到端。
//!
//! 锁定的合同事实：
//! - `f64.to_bits` / `f64.from_bits` 是结构性 opcode 映射（`i64.reinterpret_f64` /
//!   `f64.reinterpret_i64`），不按函数名特判；
//! - 往返必须逐位恒等，覆盖 `-0.0`、次正规数与 NaN payload；
//! - `f32` 目前仍无目标合同，位重解释声明只能失败闭合并在计划期点名
//!   `Float { bits: 32 }`；
//! - 类型错误的负例必须在编译期结构化失败，不能静默把 `u32` 位当 `u64` 用。
#![cfg(feature = "ovm-lane")]

use nyar::TheoryBundle;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const CONTRACTS: &str = r#"
namespace demo.primitive.f64_bits;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::u64")]
structure u64 { }

[primitive("std::primitive::f64")]
structure f64 { }

[host_contract]
micro __f64_to_bits(value: f64): u64

[host_contract]
micro __f64_from_bits(bits: u64): f64

[host_contract]
micro __f64_neg(value: f64): f64
"#;

const PROVIDERS: &str = r#"
namespace demo.adaptor.f64_bits;

[host_provider(demo::primitive::f64_bits::__f64_to_bits)]
[wasm_operator("i64.reinterpret_f64")]
micro f64_to_bits(value: f64): u64

[host_provider(demo::primitive::f64_bits::__f64_from_bits)]
[wasm_operator("f64.reinterpret_i64")]
micro f64_from_bits(bits: u64): f64

[host_provider(demo::primitive::f64_bits::__f64_neg)]
[wasm_operator("f64.neg")]
micro f64_neg(value: f64): f64
"#;

const F32_CONTRACTS: &str = r#"
namespace demo.primitive.f32_bits;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u32")]
structure u32 { }

[primitive("std::primitive::u64")]
structure u64 { }

[primitive("std::primitive::f32")]
structure f32 { }

[host_contract]
micro __f32_to_bits(value: f32): u32

[host_contract]
micro __f32_from_bits(bits: u32): f32
"#;

const F32_PROVIDERS: &str = r#"
namespace demo.adaptor.f32_bits;

[host_provider(demo::primitive::f32_bits::__f32_to_bits)]
[wasm_operator("i32.reinterpret_f32")]
micro f32_to_bits(value: f32): u32

[host_provider(demo::primitive::f32_bits::__f32_from_bits)]
[wasm_operator("f32.reinterpret_i32")]
micro f32_from_bits(bits: u32): f32
"#;

fn bindings(
    ops_namespace: [&str; 3],
    adaptor_namespace: [&str; 3],
    pairs: &[(&str, &str)],
) -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let ops_ns = ops_namespace.map(str::to_owned);
    let adaptor_ns = adaptor_namespace.map(str::to_owned);
    pairs
        .iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &ops_ns, &format!("__{contract}")),
            provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn f64_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    bindings(
        ["demo", "primitive", "f64_bits"],
        ["demo", "adaptor", "f64_bits"],
        &[("f64_to_bits", "f64_to_bits"), ("f64_from_bits", "f64_from_bits"), ("f64_neg", "f64_neg")],
    )
}

fn f32_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    bindings(
        ["demo", "primitive", "f32_bits"],
        ["demo", "adaptor", "f32_bits"],
        &[("f32_to_bits", "f32_to_bits"), ("f32_from_bits", "f32_from_bits")],
    )
}

fn f64_groups(return_type: &str, statements: &str) -> Vec<CompilerSourceGroup> {
    let app = format!("namespace demo.primitive.f64_bits;\n\n[main]\nmicro main() -> {return_type} {{\n{statements}\n}}\n");
    vec![
        CompilerSourceGroup::single_source("core", "core", CONTRACTS),
        CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDERS).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", &app).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

fn f32_groups(app: &str) -> Vec<CompilerSourceGroup> {
    let app = format!("namespace demo.primitive.f32_bits;\n\n[main]\nmicro main() -> u64 {{\n{app}\n}}\n");
    vec![
        CompilerSourceGroup::single_source("core", "core", F32_CONTRACTS),
        CompilerSourceGroup::single_source("adaptor", "adaptor", F32_PROVIDERS).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", &app).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

fn compile_f64(return_type: &str, statements: &str) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&f64_groups(return_type, statements), &f64_bindings(), &[], "native")
        .map_err(|error| error.to_string())
}

fn plan_for(program: &nyar_types::CompiledProgram) -> Result<TargetExecutionPlan, String> {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, TheoryBundle::default(), &roots).map_err(|error| error.to_string())
}

fn lower_wasm(return_type: &str, statements: &str) -> Result<Vec<u8>, String> {
    let program = compile_f64(return_type, statements)?;
    let plan = plan_for(&program)?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())?
        .to_bytes()
        .map_err(|error| error.to_string())
}

/// 在 Node 里实例化并读取 `main` 的 i64 返回值，按无符号 64 位十进制打印。
fn execute_wasm_bits(bytes: &[u8]) -> u64 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("f64_bits_probe.wasm");
    let script_path = dir.path().join("run.mjs");
    std::fs::write(&wasm_path, bytes).expect("write wasm");
    std::fs::write(
        &script_path,
        r#"
import { readFileSync } from "node:fs";
const buf = readFileSync(process.argv[2]);
const mod = await WebAssembly.compile(buf);
const importObject = {};
for (const imp of WebAssembly.Module.imports(mod)) {
  if (!importObject[imp.module]) importObject[imp.module] = {};
  if (imp.kind === "function") {
    importObject[imp.module][imp.name] = () => 0;
  } else if (imp.kind === "global") {
    importObject[imp.module][imp.name] = new WebAssembly.Global({ value: "i64", mutable: true }, 0n);
  } else if (imp.kind === "memory") {
    importObject[imp.module][imp.name] = new WebAssembly.Memory({ initial: 1 });
  } else if (imp.kind === "table") {
    importObject[imp.module][imp.name] = new WebAssembly.Table({ initial: 0, element: "anyfunc" });
  }
}
const inst = await WebAssembly.instantiate(mod, importObject);
const fn = inst.exports.main;
if (typeof fn !== "function") {
  console.error("missing main export");
  process.exit(2);
}
process.stdout.write(BigInt.asUintN(64, fn()).toString());
"#,
    )
    .expect("write script");
    let output = std::process::Command::new("node").arg(&script_path).arg(&wasm_path).output().expect("spawn node to execute wasm");
    if !output.status.success() {
        panic!("node wasm execute failed status={:?}\nstderr={}", output.status.code(), String::from_utf8_lossy(&output.stderr));
    }
    String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().expect("wasm main 必须返回 64 位位模式")
}

fn run_bits(statements: &str) -> u64 {
    let bytes = lower_wasm("u64", statements).expect("f64 位重解释必须完成 wasm 降低");
    execute_wasm_bits(&bytes)
}

/// 往返恒等：`from_bits(to_bits(x))` 与 `to_bits(from_bits(bits))` 都必须逐位相同。
#[test]
fn f64_bits_round_trip_is_exact_for_normal_values() {
    // 位模式字面量必须是 i64 可表示的无符号位模式；符号位为 1 的值用 `from_bits` 的反方向覆盖。
    for bits in [1.5f64.to_bits(), 3.75f64.to_bits(), f64::MIN_POSITIVE.to_bits()] {
        assert_eq!(run_bits(&format!("    return __f64_to_bits(__f64_from_bits({bits}))")), bits, "位模式必须往返恒等");
    }
    // 字面量方向只用词法可精确表示的十进制，避免把科学记数法词法缺陷算进载体测试。
    for value in [1.5f64, 3.75] {
        assert_eq!(
            run_bits(&format!("    let value: f64 = {value}\n    return __f64_to_bits(value)")),
            value.to_bits(),
            "`{value}` 的 `to_bits` 必须与宿主 Rust 位模式一致"
        );
    }
}

/// `-0.0` 的符号位必须保留，不能被折叠成 `+0.0`。
#[test]
fn f64_negative_zero_keeps_its_sign_bit() {
    let expected = (-0.0f64).to_bits();
    assert_eq!(run_bits("    return __f64_to_bits(__f64_neg(0.0))"), expected, "`-0.0` 的符号位必须保留");
    // `0x8000000000000000` 超出 i64 字面量范围，这里用 `neg(0.0)` 产生 -0.0 再走 from_bits 往返。
    assert_eq!(
        run_bits("    return __f64_to_bits(__f64_from_bits(__f64_to_bits(__f64_neg(0.0))))"),
        expected,
        "从位模式重建的 `-0.0` 必须保留符号位"
    );
}

/// 最小次正规数 `0x1` 必须逐位保留。
#[test]
fn f64_subnormal_bit_pattern_is_preserved() {
    let expected = f64::from_bits(1).to_bits();
    assert_eq!(expected, 1, "最小次正规数的位模式必须是 1");
    assert_eq!(run_bits("    return __f64_to_bits(__f64_from_bits(1))"), expected, "次正规数位模式必须往返恒等");
}

/// NaN payload 不得塌成固定规范值：带 payload 的 qNaN / sNaN 都必须逐位往返。
#[test]
fn f64_nan_payload_is_preserved() {
    for bits in [0x7ff8_0000_0000_0001u64, 0x7ff0_0000_0000_0001u64] {
        assert_eq!(run_bits(&format!("    return __f64_to_bits(__f64_from_bits({bits}))")), bits, "NaN payload 必须逐位保留");
    }
}

/// 结构性映射：两个方向都必须走 `Wasm` 单指令，而不是宿主导入。
#[test]
fn f64_bits_lower_to_inline_wasm_opcodes() {
    let program = compile_f64("u64", "    return __f64_to_bits(__f64_from_bits(1))").expect("编译");
    let plan = plan_for(&program).expect("计划");
    let submission = FragmentSubmission::from_backend_plan(plan);
    let module = lower_fragment_to_wasm_mir_module(&submission, "main").expect("wasm 降低");
    let bytes = module.to_bytes().expect("wasm 编码");
    assert!(bytes.contains(&0xBD), "必须包含 `i64.reinterpret_f64` (0xBD)");
    assert!(bytes.contains(&0xBF), "必须包含 `f64.reinterpret_i64` (0xBF)");
}

/// OVM 没有位重解释目标 provider：必须结构化失败，不得 panic 或回落为宿主导入。
#[test]
fn f64_bits_fail_closed_on_ovm_target() {
    use nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module;
    let program = compile_f64("u64", "    return __f64_to_bits(__f64_from_bits(1))").expect("编译");
    let plan = plan_for(&program).expect("计划");
    let error = lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map(|_| ())
        .expect_err("OVM 缺少位重解释 provider，必须结构化失败");
    let error = error.to_string();
    assert!(error.contains("未绑定目标 provider"), "诊断必须点名未绑定目标 provider，实际：{error}");
    assert!(error.contains("wasm-operator"), "诊断必须暴露实际链接目标标签，实际：{error}");
}

/// 类型错误负例：`f32.to_bits` 的 `u32` 结果不得静默当 `u64` 用。
#[test]
fn f32_bits_cannot_be_used_as_u64() {
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(
            &f32_groups("    let value: f32 = __f32_from_bits(1)\n    let wrong: u64 = __f32_to_bits(value)\n    return wrong"),
            &f32_bindings(),
            &[],
            "native",
        )
        .expect_err("`u32` 位当 `u64` 用必须结构化失败");
    let error = error.to_string();
    assert!(
        error.contains("u32") && error.contains("u64") || error.contains("类型") || error.contains("type"),
        "类型错误诊断必须点名两侧类型，实际：{error}"
    );
}

/// `f32` 仍无目标合同：位重解释声明只能失败闭合，并在计划期点名 `Float { bits: 32 }`。
#[test]
fn f32_bits_fail_closed_at_the_unregistered_primitive() {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(
            &f32_groups("    let value: f32 = __f32_from_bits(1)\n    let bits: u32 = __f32_to_bits(value)\n    return 0"),
            &f32_bindings(),
            &[],
            "native",
        )
        .expect("`f32` 源必须能编译到 canonical");
    let error = plan_for(&program).expect_err("`f32` 本轮仍无目标合同，必须失败闭合");
    assert!(error.contains("Float { bits: 32 }"), "诊断必须点名未登记的 f32 目标合同，实际：{error}");
    assert!(error.contains("primitive 尚无目标合同"), "诊断必须是计划期结构化失败，实际：{error}");
}
