//! `f64` / `f32` 位模式重解释与 f32 独立载体的原始类型合同端到端。
//!
//! 锁定的合同事实：
//! - `f64.to_bits` / `f64.from_bits` 是结构性 opcode 映射（`i64.reinterpret_f64` /
//!   `f64.reinterpret_i64`），不按函数名特判；
//! - `f32` 有**独立载体**：wasm 侧 `VALTYPE_F32`（`0x7D`）与 `i32.reinterpret_f32` /
//!   `f32.reinterpret_i32`，OVM 侧 `Value::F32` 与头码 `0x62` / `0x63`；
//! - 往返必须逐位恒等，覆盖 `-0.0`、次正规数与 NaN payload；
//! - `f32` 算术在单精度上完成，`f64 ↔ f32` 转换是单步 RNE/精确提升，不得借槽；
//! - 类型错误的负例必须在编译期结构化失败：`f32` 静默拓宽到 `f64`、`f32` 位当
//!   `u64` 用都必须失败。
#![cfg(feature = "ovm-lane")]

use nyar::TheoryBundle;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

// primitive 身份门：载体声明必须落在 `namespace std.primitive;`，身份引用才能
// 在携带者可见面内解析；demo 名空间经 `using std.primitive;` 引用载体类型。
const STD_PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::u32")]
structure u32 { }

[primitive("std::primitive::u64")]
structure u64 { }

[primitive("std::primitive::f32")]
structure f32 { }

[primitive("std::primitive::f64")]
structure f64 { }
"#;

const CONTRACTS: &str = r#"
using std.primitive;

namespace demo.primitive.f64_bits;

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
using std.primitive;

namespace demo.primitive.f32_bits;

[host_contract]
micro __f32_to_bits(value: f32): u32

[host_contract]
micro __f32_from_bits(bits: u32): f32

[host_contract]
micro __f32_neg(value: f32): f32

[host_contract]
micro __f32_add(lhs: f32, rhs: f32): f32

[host_contract]
micro __f64_from_bits(bits: u64): f64

[host_contract]
micro __f64_to_bits(value: f64): u64
"#;

const F32_PROVIDERS: &str = r#"
namespace demo.adaptor.f32_bits;

[host_provider(demo::primitive::f32_bits::__f32_to_bits)]
[wasm_operator("i32.reinterpret_f32")]
micro f32_to_bits(value: f32): u32

[host_provider(demo::primitive::f32_bits::__f32_from_bits)]
[wasm_operator("f32.reinterpret_i32")]
micro f32_from_bits(bits: u32): f32

[host_provider(demo::primitive::f32_bits::__f32_neg)]
[wasm_operator("f32.neg")]
micro f32_neg(value: f32): f32

[host_provider(demo::primitive::f32_bits::__f32_add)]
[wasm_operator("f32.add")]
micro f32_add(lhs: f32, rhs: f32): f32

[host_provider(demo::primitive::f32_bits::__f64_from_bits)]
[wasm_operator("f64.reinterpret_i64")]
micro f64_from_bits(bits: u64): f64

[host_provider(demo::primitive::f32_bits::__f64_to_bits)]
[wasm_operator("i64.reinterpret_f64")]
micro f64_to_bits(value: f64): u64
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
        &[
            ("f32_to_bits", "f32_to_bits"),
            ("f32_from_bits", "f32_from_bits"),
            ("f32_neg", "f32_neg"),
            ("f32_add", "f32_add"),
            ("f64_from_bits", "f64_from_bits"),
            ("f64_to_bits", "f64_to_bits"),
        ],
    )
}

fn f64_groups(return_type: &str, statements: &str) -> Vec<CompilerSourceGroup> {
    f64_groups_with(return_type, statements, PROVIDERS)
}

fn f64_groups_with(return_type: &str, statements: &str, providers: &str) -> Vec<CompilerSourceGroup> {
    let app = format!("using std.primitive;\n\nnamespace demo.primitive.f64_bits;\n\n[main]\nmicro main() -> {return_type} {{\n{statements}\n}}\n");
    vec![
        CompilerSourceGroup::single_source("std", "std", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("core", "core", CONTRACTS).with_direct_dependencies(vec!["std".into()]),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["std".into(), "core".into()]),
        CompilerSourceGroup::single_source("app", "demo", &app).as_entry().with_direct_dependencies(vec!["std".into(), "core".into(), "adaptor".into()]),
    ]
}

fn f32_groups(statements: &str) -> Vec<CompilerSourceGroup> {
    f32_groups_with(statements, F32_PROVIDERS)
}

fn f32_groups_with(statements: &str, providers: &str) -> Vec<CompilerSourceGroup> {
    f32_groups_with_return(statements, providers, "u32")
}

fn f32_groups_with_return(statements: &str, providers: &str, return_type: &str) -> Vec<CompilerSourceGroup> {
    let app = format!("using std.primitive;\n\nnamespace demo.primitive.f32_bits;\n\n[main]\nmicro main() -> {return_type} {{\n{statements}\n}}\n");
    vec![
        CompilerSourceGroup::single_source("std", "std", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("core", "core", F32_CONTRACTS).with_direct_dependencies(vec!["std".into()]),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["std".into(), "core".into()]),
        CompilerSourceGroup::single_source("app", "demo", &app).as_entry().with_direct_dependencies(vec!["std".into(), "core".into(), "adaptor".into()]),
    ]
}

fn compile_f64(return_type: &str, statements: &str) -> Result<nyar_types::CompiledProgram, String> {
    compile_f64_with(return_type, statements, PROVIDERS)
}

fn compile_f64_with(return_type: &str, statements: &str, providers: &str) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(
            &f64_groups_with(return_type, statements, providers),
            &f64_bindings(),
            &[],
            "native",
        )
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

/// 只有 wasm provider、没有 OVM provider：OVM 必须结构化失败，不得 panic 或回落为宿主导入。
#[test]
fn f64_bits_without_ovm_provider_fail_closed() {
    use nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module;
    let program = compile_f64("u64", "    return __f64_to_bits(__f64_from_bits(1))").expect("编译");
    let plan = plan_for(&program).expect("计划");
    let error = lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map(|_| ())
        .expect_err("OVM 缺少 `[ovm_operator]` provider，必须结构化失败");
    let error = error.to_string();
    assert!(error.contains("未绑定目标 provider"), "诊断必须点名未绑定目标 provider，实际：{error}");
    assert!(error.contains("wasm-operator"), "诊断必须暴露实际链接目标标签，实际：{error}");
}

/// OVM 位重解释：`[ovm_operator]` 冻结为 `I64ReinterpretF64` / `F64ReinterpretI64`
/// 头码，在 OVM 解释器上真实执行；同一份源码必须与 wasm 给出同一位模式。
#[cfg(feature = "ovm-lane")]
mod ovm {
    use super::*;
    use acorn_odin::{OdinExportKind, encode_module};
    use odin_vm::{executor::Executor, module::LoadedModule, value::Value};

    /// 与 `std.adaptor.ovm` 正式形制一致：位重解释走 `[ovm_operator]`，
    /// `f64_neg` 仍走既有 `[ovm]` 宿主导入（用于制造 `-0.0`）。
    const OVM_PROVIDERS: &str = r#"
namespace demo.adaptor.f64_bits;

[host_provider(demo::primitive::f64_bits::__f64_to_bits)]
[ovm_operator("i64.reinterpret_f64")]
micro f64_to_bits(value: f64): u64

[host_provider(demo::primitive::f64_bits::__f64_from_bits)]
[ovm_operator("f64.reinterpret_i64")]
micro f64_from_bits(bits: u64): f64

[host_provider(demo::primitive::f64_bits::__f64_neg)]
[ovm("f64_neg")]
micro f64_neg(value: f64): f64
"#;

    fn lower_ovm(statements: &str) -> Result<acorn_odin::OdinModuleData, String> {
        let program = compile_f64_with("u64", statements, OVM_PROVIDERS)?;
        let plan = plan_for(&program)?;
        nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .map_err(|error| error.to_string())
    }

    /// 在 OVM 解释器里执行 `main`，返回 64 位位模式。
    fn run_ovm_bits(statements: &str) -> u64 {
        let module = lower_ovm(statements).expect("OVM 位重解释必须完成降低");
        let bytes = encode_module(&module);
        let loaded = LoadedModule::from_bytes(&bytes).expect("位重解释模块必须通过加载校验");
        let entry = loaded
            .exports
            .iter()
            .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with("main"))
            .expect("导出表必须包含 main");
        let mut executor = Executor::new();
        match executor.run(&loaded, entry.function_index as usize, Vec::new()).expect("OVM 必须完成位重解释") {
            Value::I64(bits) => bits as u64,
            other => panic!("`to_bits` 必须返回 i64 位模式，实际 {other:?}"),
        }
    }

    /// 普通值 / 次正规数 / 最小正规数：往返位模式与 wasm 逐位一致。
    #[test]
    fn f64_bits_round_trip_matches_wasm() {
        let cases = [
            1.5f64.to_bits(),
            3.75f64.to_bits(),
            // 最小次正规数（5e-324）与最大次正规数。
            1,
            0x000f_ffff_ffff_ffff,
            f64::MIN_POSITIVE.to_bits(),
        ];
        for bits in cases {
            let source = format!("    return __f64_to_bits(__f64_from_bits({bits}))");
            assert_eq!(run_ovm_bits(&source), bits, "OVM 往返必须位模式恒等（bits={bits:#x}）");
            assert_eq!(run_ovm_bits(&source), run_bits(&source), "OVM 与 wasm 必须给出同一位模式（bits={bits:#x}）");
        }
    }

    /// `-0.0` 的符号位在两个 target 上都必须保留。
    #[test]
    fn f64_negative_zero_matches_wasm() {
        let expected = (-0.0f64).to_bits();
        for source in ["    return __f64_to_bits(__f64_neg(0.0))", "    return __f64_to_bits(__f64_from_bits(__f64_to_bits(__f64_neg(0.0))))"] {
            assert_eq!(run_ovm_bits(source), expected, "OVM 必须保留 `-0.0` 的符号位");
            assert_eq!(run_ovm_bits(source), run_bits(source), "OVM 与 wasm 的 `-0.0` 位模式必须一致");
        }
    }

    /// NaN payload 不得塌成固定规范值：qNaN / sNaN 在两个 target 上逐位一致。
    #[test]
    fn f64_nan_payload_matches_wasm() {
        for bits in [0x7ff8_0000_0000_0001u64, 0x7ff0_0000_0000_0001u64] {
            let source = format!("    return __f64_to_bits(__f64_from_bits({bits}))");
            assert_eq!(run_ovm_bits(&source), bits, "OVM 必须逐位保留 NaN payload（bits={bits:#x}）");
            assert_eq!(run_ovm_bits(&source), run_bits(&source), "OVM 与 wasm 的 NaN payload 必须一致（bits={bits:#x}）");
        }
    }

    /// 结构性事实：位重解释必须是头码 0x60/0x61，不得产生任何 `nyar.host` 导入
    /// （即不新增 capability）。
    ///
    /// 位模式字面量取 1.5 的 64 位值（超出 i32），确保常量走 `Integer64` 池，
    /// 不掺入既有 `i32_to_i64` 载体辅助导入，从而严格断言 reinterpret 路径零导入。
    #[test]
    fn f64_bits_use_ovm_head_codes_without_host_imports() {
        let module = lower_ovm("    return __f64_to_bits(__f64_from_bits(4609434218613702656))").expect("OVM 位重解释必须完成降低");
        assert!(module.code_bytes.contains(&0x60), "必须发射 `I64ReinterpretF64` (0x60)");
        assert!(module.code_bytes.contains(&0x61), "必须发射 `F64ReinterpretI64` (0x61)");
        assert!(module.imports.is_empty(), "位重解释不得引入宿主导入/capability: {:?}", module.imports);
    }

    /// 与 `std.adaptor.ovm` 正式形制一致：f32 位重解释与算术全部走 `[ovm_operator]` 头码，
    /// 不引入宿主函数/capability。
    const OVM_F32_PROVIDERS: &str = r#"
namespace demo.adaptor.f32_bits;

[host_provider(demo::primitive::f32_bits::__f32_to_bits)]
[ovm_operator("i32.reinterpret_f32")]
micro f32_to_bits(value: f32): u32

[host_provider(demo::primitive::f32_bits::__f32_from_bits)]
[ovm_operator("f32.reinterpret_i32")]
micro f32_from_bits(bits: u32): f32

[host_provider(demo::primitive::f32_bits::__f32_neg)]
[ovm_operator("f32.neg")]
micro f32_neg(value: f32): f32

[host_provider(demo::primitive::f32_bits::__f32_add)]
[ovm_operator("f32.add")]
micro f32_add(lhs: f32, rhs: f32): f32

[host_provider(demo::primitive::f32_bits::__f64_from_bits)]
[ovm_operator("f64.reinterpret_i64")]
micro f64_from_bits(bits: u64): f64

[host_provider(demo::primitive::f32_bits::__f64_to_bits)]
[ovm_operator("i64.reinterpret_f64")]
micro f64_to_bits(value: f64): u64
"#;

    fn lower_ovm_f32(statements: &str) -> Result<acorn_odin::OdinModuleData, String> {
        let program = compile_f32_with(statements, OVM_F32_PROVIDERS)?;
        let plan = plan_for(&program)?;
        nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .map_err(|error| error.to_string())
    }

    /// 在 OVM 解释器里执行 `main`，返回 32 位位模式。
    fn run_ovm_bits32(statements: &str) -> u32 {
        let module = lower_ovm_f32(statements).expect("OVM f32 必须完成降低");
        let bytes = encode_module(&module);
        let loaded = LoadedModule::from_bytes(&bytes).expect("f32 模块必须通过加载校验");
        let entry = loaded
            .exports
            .iter()
            .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with("main"))
            .expect("导出表必须包含 main");
        let mut executor = Executor::new();
        match executor.run(&loaded, entry.function_index as usize, Vec::new()).expect("OVM 必须完成 f32 位重解释") {
            Value::I32(bits) => bits as u32,
            other => panic!("`f32.to_bits` 必须返回 i32 位模式，实际 {other:?}"),
        }
    }

    /// 两个 target 的 f32 位模式必须逐位完全相同（0.1f32 / MIN_POSITIVE / 次正规 / MAX / NaN payload）。
    #[test]
    fn f32_bits_round_trip_matches_wasm() {
        for bits in f32_bit_pattern_cases() {
            let source = format!("    return __f32_to_bits(__f32_from_bits({bits}))");
            assert_eq!(run_ovm_bits32(&source), bits, "OVM f32 往返必须位模式恒等（bits={bits:#x}）");
            assert_eq!(run_ovm_bits32(&source), run_bits32(&source), "OVM 与 wasm 必须给出同一位模式（bits={bits:#x}）");
        }
    }

    /// `-0.0` 与 f32 算术在两个 target 上逐位一致。
    #[test]
    fn f32_negative_zero_and_arithmetic_match_wasm() {
        let neg_source = "    return __f32_to_bits(__f32_neg(__f32_from_bits(0)))";
        assert_eq!(run_ovm_bits32(neg_source), 0x8000_0000, "OVM 必须保留 `-0.0` 的符号位");
        assert_eq!(run_ovm_bits32(neg_source), run_bits32(neg_source), "OVM 与 wasm 的 `-0.0` 位模式必须一致");

        let add_source =
            format!("    return __f32_to_bits(__f32_add(__f32_from_bits({}), __f32_from_bits({})))", 0.1f32.to_bits(), 0.2f32.to_bits());
        assert_eq!(run_ovm_bits32(&add_source), (0.1f32 + 0.2f32).to_bits(), "OVM 必须在单精度上做加法");
        assert_eq!(run_ovm_bits32(&add_source), run_bits32(&add_source), "OVM 与 wasm 的 f32 加法必须逐位一致");
    }

    /// OVM 的 `f64 → f32` 收窄 / `f32 → f64` 提升与 wasm 逐位一致（含半途、溢出、下溢）。
    #[test]
    fn f32_conversions_match_wasm() {
        let cases = [
            1.0f64 + 2.0f64.powi(-24),
            1.0f64 + 2.0f64.powi(-24) + 2.0f64.powi(-40),
            f64::MAX,
            2.0f64.powi(-150),
            2.0f64.powi(-149),
        ];
        for value in cases {
            let source = format!("    return __f32_to_bits(__f64_from_bits({}) as f32)", value.to_bits());
            assert_eq!(run_ovm_bits32(&source), (value as f32).to_bits(), "OVM 收窄必须是单步 RNE（bits={:#x}）", value.to_bits());
            assert_eq!(run_ovm_bits32(&source), run_bits32(&source), "OVM 与 wasm 的收窄位模式必须一致");
        }
        for bits in f32_bit_pattern_cases() {
            let expected = (f32::from_bits(bits) as f64).to_bits();
            let source = f32_promote_source(bits);
            assert_eq!(run_ovm_f32_bits(&source), expected, "OVM 提升必须精确（bits={bits:#x}）");
            assert_eq!(run_ovm_f32_bits(&source), run_f32_bits64(&source), "OVM 与 wasm 的提升位模式必须一致");
        }
    }

    /// OVM 侧独立载体：位重解释是头码 0x62 / 0x63，算术是 0x64 起的头码，零宿主导入。
    #[test]
    fn f32_bits_use_ovm_head_codes_without_host_imports() {
        let module = lower_ovm_f32("    return __f32_to_bits(__f32_from_bits(1))").expect("OVM f32 必须完成降低");
        assert!(module.code_bytes.contains(&0x62), "必须发射 `I32ReinterpretF32` (0x62)");
        assert!(module.code_bytes.contains(&0x63), "必须发射 `F32ReinterpretI32` (0x63)");
        assert!(module.imports.is_empty(), "f32 位重解释不得引入宿主导入/capability: {:?}", module.imports);
    }

    /// f32→f64 提升源码（单独提出便于与 wasm 位模式对齐）。
    fn f32_promote_source(bits: u32) -> String {
        format!("    return __f64_to_bits(__f32_from_bits({bits}) as f64)")
    }

    fn lower_ovm_f32_with_return(statements: &str, return_type: &str) -> Result<acorn_odin::OdinModuleData, String> {
        let program = ValkyrieCompiler::default()
            .compile_source_groups_to_program_with_host_bindings(
                &f32_groups_with_return(statements, OVM_F32_PROVIDERS, return_type),
                &f32_bindings(),
                &[],
                "native",
            )
            .map_err(|error| error.to_string())?;
        let plan = plan_for(&program)?;
        nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .map_err(|error| error.to_string())
    }

    /// 在 OVM 解释器里执行 `main`，返回 64 位位模式（用于 f32→f64 提升）。
    fn run_ovm_f32_bits(statements: &str) -> u64 {
        let module = lower_ovm_f32_with_return(statements, "u64").expect("OVM f32 转换必须完成降低");
        let bytes = encode_module(&module);
        let loaded = LoadedModule::from_bytes(&bytes).expect("f32 转换模块必须通过加载校验");
        let entry = loaded
            .exports
            .iter()
            .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with("main"))
            .expect("导出表必须包含 main");
        let mut executor = Executor::new();
        match executor.run(&loaded, entry.function_index as usize, Vec::new()).expect("OVM 必须完成 f32→f64 提升") {
            Value::I64(bits) => bits as u64,
            other => panic!("`f64.to_bits` 必须返回 i64 位模式，实际 {other:?}"),
        }
    }
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

fn compile_f32(statements: &str) -> Result<nyar_types::CompiledProgram, String> {
    compile_f32_with(statements, F32_PROVIDERS)
}

fn compile_f32_with(statements: &str, providers: &str) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&f32_groups_with(statements, providers), &f32_bindings(), &[], "native")
        .map_err(|error| error.to_string())
}

fn lower_wasm32(statements: &str) -> Result<Vec<u8>, String> {
    let program = compile_f32(statements)?;
    let plan = plan_for(&program)?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())?
        .to_bytes()
        .map_err(|error| error.to_string())
}

/// 在 Node 里实例化并读取 `main` 的 i32 返回值，按无符号 32 位十进制打印。
fn execute_wasm_bits32(bytes: &[u8]) -> u32 {
    let dir = tempfile::tempdir().expect("tempdir");
    let wasm_path = dir.path().join("f32_bits_probe.wasm");
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
process.stdout.write((fn() >>> 0).toString());
"#,
    )
    .expect("write script");
    let output = std::process::Command::new("node").arg(&script_path).arg(&wasm_path).output().expect("spawn node to execute wasm");
    if !output.status.success() {
        panic!("node wasm execute failed status={:?}\nstderr={}", output.status.code(), String::from_utf8_lossy(&output.stderr));
    }
    String::from_utf8_lossy(&output.stdout).trim().parse::<u32>().expect("wasm main 必须返回 32 位位模式")
}

fn run_bits32(statements: &str) -> u32 {
    let bytes = lower_wasm32(statements).expect("f32 位重解释必须完成 wasm 降低");
    execute_wasm_bits32(&bytes)
}

/// `main() -> u64` 形态：用于 f32→f64 提升后的 64 位位模式取证。
fn lower_wasm64_f32(statements: &str) -> Result<Vec<u8>, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(
            &f32_groups_with_return(statements, F32_PROVIDERS, "u64"),
            &f32_bindings(),
            &[],
            "native",
        )
        .map_err(|error| error.to_string())?;
    let plan = plan_for(&program)?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())?
        .to_bytes()
        .map_err(|error| error.to_string())
}

fn run_f32_bits64(statements: &str) -> u64 {
    let bytes = lower_wasm64_f32(statements).expect("f32→f64 提升必须完成 wasm 降低");
    execute_wasm_bits(&bytes)
}

/// 位模式一致性用例：0.1f32、最小正规数、最大/最小次正规数、f32::MAX 与两种 NaN payload。
fn f32_bit_pattern_cases() -> Vec<u32> {
    vec![
        // 0.1f32 的真实位模式（十进制 0.1 无法精确表示）。
        0x3dcc_cccd,
        // f32::MIN_POSITIVE（最小正规数）。
        0x0080_0000,
        // 最小次正规数与最大次正规数。
        0x0000_0001,
        0x007f_ffff,
        // f32::MAX。
        0x7f7f_ffff,
        // qNaN payload 与带非零 payload 的 NaN（payload 23 位必须保位）。
        0x7f80_0001,
        0x7fc0_0123,
    ]
}

/// 逐位往返恒等：`to_bits(from_bits(bits)) == bits`，覆盖 0.1f32 / MIN_POSITIVE /
/// 最大最小次正规 / MAX / NaN payload。
#[test]
fn f32_bits_round_trip_is_exact() {
    for bits in f32_bit_pattern_cases() {
        let source = format!("    return __f32_to_bits(__f32_from_bits({bits}))");
        assert_eq!(run_bits32(&source), bits, "f32 位模式必须往返恒等（bits={bits:#x}）");
    }
}

/// `-0.0` 的符号位必须保留，不能被折叠成 `+0.0`；次正规位模式的符号位同样保位。
#[test]
fn f32_negative_zero_keeps_its_sign_bit() {
    let expected = (-0.0f32).to_bits();
    assert_eq!(expected, 0x8000_0000, "`-0.0f32` 的位模式必须是 `0x80000000`");
    assert_eq!(
        run_bits32("    return __f32_to_bits(__f32_neg(__f32_from_bits(0)))"),
        expected,
        "`f32.neg(0.0)` 必须产生 `-0.0`"
    );
    assert_eq!(
        run_bits32("    return __f32_to_bits(__f32_from_bits(__f32_to_bits(__f32_neg(__f32_from_bits(0)))))"),
        expected,
        "从位模式重建的 `-0.0` 必须保留符号位"
    );
}

/// 算术必须逐位对齐宿主 Rust 的 f32 运算（单精度、不提升到 f64）。
#[test]
fn f32_arithmetic_is_bit_exact() {
    let add = 0.1f32.to_bits();
    let add_rhs = 0.2f32.to_bits();
    assert_eq!(
        run_bits32(&format!("    return __f32_to_bits(__f32_add(__f32_from_bits({add}), __f32_from_bits({add_rhs})))")),
        (0.1f32 + 0.2f32).to_bits(),
        "`0.1f32 + 0.2f32` 必须与宿主逐位一致（次正规/舍入边界不能被 f64 中转改写）"
    );
}

/// `f64 → f32` 是单步 round-to-nearest-even：刚好半途取偶数、溢出到 `inf`、
/// 下溢到 0 或最小次正规。
#[test]
fn f32_demote_from_f64_rounds_to_nearest_even() {
    let cases = [
        // 半途（1 + 2^-24），RNE 取偶数尾数 → 1.0。
        1.0f64 + 2.0f64.powi(-24),
        // 略高于半途 → 进位到下一个 f32。
        1.0f64 + 2.0f64.powi(-24) + 2.0f64.powi(-40),
        // 略低于半途 → 仍取偶数 1.0。
        1.0f64 + 2.0f64.powi(-24) - 2.0f64.powi(-40),
        // 溢出：f64::MAX 收窄后必须是 `inf`。
        f64::MAX,
        // 刚好半途于 0 与最小次正规之间 → 偶数 0。
        2.0f64.powi(-150),
        // 最小次正规本身 → 位模式 1。
        2.0f64.powi(-149),
    ];
    for value in cases {
        let bits = value.to_bits();
        let expected = (value as f32).to_bits();
        assert_eq!(
            run_bits32(&format!("    return __f32_to_bits(__f64_from_bits({bits}) as f32)")),
            expected,
            "`{value}` → f32 必须是单步 RNE（bits={bits:#x}）"
        );
    }
    assert_eq!((f64::MAX as f32).to_bits(), 0x7f80_0000, "溢出必须到 `+inf`");
    assert_eq!((2.0f64.powi(-150) as f32).to_bits(), 0, "半途必须取偶数 0");
}

/// `f32 → f64` 精确无损：提升后的 64 位位模式必须与宿主一致。
#[test]
fn f32_promote_to_f64_is_exact() {
    for bits in f32_bit_pattern_cases() {
        let expected = (f32::from_bits(bits) as f64).to_bits();
        let output = run_f32_bits64(&format!("    return __f64_to_bits(__f32_from_bits({bits}) as f64)"));
        assert_eq!(output, expected, "f32→f64 必须精确（bits={bits:#x}）");
    }
}

/// 结构性事实：wasm 侧必须使用独立 f32 载体（`0x7D`）与 32 位位重解释（`0xBC` / `0xBE`）。
#[test]
fn f32_bits_lower_to_independent_wasm_carrier() {
    let program = compile_f32("    let value: f32 = __f32_from_bits(1)\n    return __f32_to_bits(value)").expect("编译");
    let plan = plan_for(&program).expect("计划");
    let bytes = lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .expect("wasm 降低")
        .to_bytes()
        .expect("wasm 编码");
    assert!(bytes.contains(&0xBC), "必须包含 `i32.reinterpret_f32` (0xBC)");
    assert!(bytes.contains(&0xBE), "必须包含 `f32.reinterpret_i32` (0xBE)");
    assert!(bytes.contains(&0x7D), "必须包含独立 `VALTYPE_F32` (0x7D)，不得借 f64 槽");
}

/// 反例：`f32` 不得静默拓宽到 `f64`（隐式拓宽不存在，必须编译期失败）。
#[test]
fn f32_does_not_silently_widen_to_f64() {
    let error = compile_f32("    let value: f32 = __f32_from_bits(1)\n    let widened: f64 = value\n    return 0")
        .expect_err("`f32` → `f64` 的隐式拓宽必须失败");
    assert!(
        error.contains("f32") && error.contains("f64") || error.contains("类型") || error.contains("type"),
        "诊断必须点名 f32/f64 载体，实际：{error}"
    );
}
