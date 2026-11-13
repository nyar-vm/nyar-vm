//! `std.primitive.__i64_{and,or,xor,shl,shr}` 位运算合同 → `[host_provider]` provider
//! → 目标指令 / `nyar.host` 宿主导入的端到端正反合同。
//!
//! 正向：合同经 wasm adaptor 的冻结配方降低为原生指令——移位实参按声明签名（双 `i64`）
//! 原样发射（Wasm `i64.shl`/`i64.shr_s` 两个操作数都是 `i64`，位移量由指令按 64 取模，
//! 不得插入 `i32.wrap_i64`）；
//! OVM lane 经 `[ovm("i64_*")]` 宿主导入降低为 `CallImport`，加载期解析为 `HostOp`
//! 并执行出正确结果。
//! 反向：缺 provider / 未知助记符必须结构化失败，不得空体成功或回落到任何旁路。
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

// primitive 身份门：载体声明必须落在 `namespace std.primitive;`。
const STD_PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }
"#;

const CONTRACTS: &str = r#"
namespace demo.primitive.i64_ops;

[host_contract]
micro __i64_and(a: i64, b: i64): i64

[host_contract]
micro __i64_or(a: i64, b: i64): i64

[host_contract]
micro __i64_xor(a: i64, b: i64): i64

[host_contract]
micro __i64_shl(a: i64, b: i64): i64

[host_contract]
micro __i64_shr(a: i64, b: i64): i64
"#;

// 与 std.adaptor.wasm 正式形制一致：`[host_provider]` + `[wasm_operator]` 直挂无体 micro。
const PROVIDERS_WASM: &str = r#"
namespace demo.adaptor.i64;

[host_provider(demo::primitive::i64_ops::__i64_and)]
[wasm_operator("i64.and")]
micro i64_and(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_or)]
[wasm_operator("i64.or")]
micro i64_or(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_xor)]
[wasm_operator("i64.xor")]
micro i64_xor(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shl)]
[wasm_operator("i64.shl")]
micro i64_shl(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shr)]
[wasm_operator("i64.shr_s")]
micro i64_shr(a: i64, b: i64): i64
"#;

// 与 std.adaptor.ovm 正式形制一致：`[host_provider]` + `[ovm]` 直挂无体 micro，
// 符号由 runtime `parse_host_op` 在加载期解析。
const PROVIDERS_OVM: &str = r#"
namespace demo.adaptor.i64;

[host_provider(demo::primitive::i64_ops::__i64_and)]
[ovm("i64_and")]
micro i64_and(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_or)]
[ovm("i64_or")]
micro i64_or(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_xor)]
[ovm("i64_xor")]
micro i64_xor(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shl)]
[ovm("i64_shl")]
micro i64_shl(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shr)]
[ovm("i64_shr")]
micro i64_shr(a: i64, b: i64): i64
"#;

// 期望值：12&10=8；8|3=11；11^1=10；10<<2=40；40>>(65&63)=40>>1=20。
const APP: &str = r#"
namespace demo.primitive.i64_ops;

[main]
micro main() -> i64 {
    let x = __i64_and(12, 10)
    let y = __i64_or(x, 3)
    let z = __i64_xor(y, 1)
    let s = __i64_shl(z, 2)
    return __i64_shr(s, 65)
}
"#;

fn provider_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let ops_ns = ["demo".to_owned(), "primitive".to_owned(), "i64_ops".to_owned()];
    let adaptor_ns = ["demo".to_owned(), "adaptor".to_owned(), "i64".to_owned()];
    [("and", "i64_and"), ("or", "i64_or"), ("xor", "i64_xor"), ("shl", "i64_shl"), ("shr", "i64_shr")]
        .into_iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &ops_ns, &format!("__i64_{contract}")),
            provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn groups_for(providers: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("std", "std", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("core", "core", CONTRACTS).with_direct_dependencies(vec!["std".into()]),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["std".into(), "core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["std".into(), "core".into(), "adaptor".into()]),
    ]
}

fn compile(
    groups: &[CompilerSourceGroup],
    bindings: &[rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding],
) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, &[], "native")
        .map_err(|error| error.to_string())
}

fn plan_for(program: &nyar_types::CompiledProgram) -> Result<TargetExecutionPlan, String> {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots).map_err(|error| error.to_string())
}

fn lower_wasm(
    groups: &[CompilerSourceGroup],
    bindings: &[rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding],
) -> Result<Vec<u8>, String> {
    let program = compile(groups, bindings)?;
    let plan = plan_for(&program)?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())?
        .to_bytes()
        .map_err(|error| error.to_string())
}

#[test]
fn i64_bitwise_wasm_provider_lowers_shift_without_narrowing() {
    let bytes = lower_wasm(&groups_for(PROVIDERS_WASM), &provider_bindings()).expect("wasm 位运算 provider 链必须完成降低");
    assert!(
        !bytes.windows(2).any(|window| window == [0xA7, 0x86].as_slice()),
        "Wasm `i64.shl` 的位移量是 `i64`，不得插入 `i32.wrap_i64`"
    );
    assert!(
        !bytes.windows(2).any(|window| window == [0xA7, 0x87].as_slice()),
        "Wasm `i64.shr_s` 的位移量是 `i64`，不得插入 `i32.wrap_i64`"
    );
    assert!(bytes.contains(&0x83) && bytes.contains(&0x84) && bytes.contains(&0x85), "i64.and/or/xor 必须落到冻结 opcode 字节");
}

#[test]
fn i64_bitwise_wasm_without_provider_fails_structurally() {
    let groups = vec![
        CompilerSourceGroup::single_source("std", "std", STD_PRIMITIVES),
        CompilerSourceGroup::single_source("core", "core", CONTRACTS).with_direct_dependencies(vec!["std".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["std".into(), "core".into()]),
    ];
    let error = lower_wasm(&groups, &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
    assert!(error.contains("demo") && error.contains("i64"), "错误必须指向未绑定 provider 的合同: {error}");
}

#[test]
fn i64_bitwise_wasm_unknown_mnemonic_fails_structurally() {
    let providers = r#"
namespace demo.adaptor.i64;

[host_provider(demo::primitive::i64_ops::__i64_and)]
[wasm_operator("i64.and")]
micro i64_and(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_or)]
[wasm_operator("i64.or")]
micro i64_or(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_xor)]
[wasm_operator("i64.xor")]
micro i64_xor(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shl)]
[wasm_operator("i64.shl")]
micro i64_shl(a: i64, b: i64): i64

[host_provider(demo::primitive::i64_ops::__i64_shr)]
[wasm_operator("i64.shr_z")]
micro i64_shr(a: i64, b: i64): i64
"#;
    let error = lower_wasm(&groups_for(providers), &provider_bindings()).expect_err("未知助记符必须落 `Unfrozen` 并在 emit 结构化失败");
    assert!(error.contains("i64.shr_z") && error.contains("可编码"), "错误必须指向未冻结的助记符: {error}");
}

#[cfg(feature = "ovm-lane")]
mod ovm {
    use super::*;
    use acorn_odin::{OdinExportKind, OdinHeadCode, OdinImportKind, encode_module};
    use odin_vm::{executor::Executor, module::LoadedModule, value::Value};

    fn lower_ovm(
        groups: &[CompilerSourceGroup],
        bindings: &[rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding],
    ) -> Result<acorn_odin::OdinModuleData, String> {
        let program = compile(groups, bindings)?;
        let plan = plan_for(&program)?;
        nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
            .map_err(|error| error.to_string())
    }

    #[test]
    fn i64_bitwise_ovm_provider_chain_executes_in_ovm() {
        let module = lower_ovm(&groups_for(PROVIDERS_OVM), &provider_bindings()).expect("OVM 位运算 provider 链必须完成降低");
        for symbol in ["i64_and", "i64_or", "i64_xor", "i64_shl", "i64_shr"] {
            assert!(
                module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == symbol),
                "`{symbol}` 必须保留 nyar.host 导入: {:?}",
                module.imports
            );
        }
        assert!(module.code_bytes.windows(1).any(|window| window[0] == OdinHeadCode::CallImport as u8), "位运算调用必须发射 CallImport");

        let bytes = encode_module(&module);
        let loaded = LoadedModule::from_bytes(&bytes).expect("已绑定宿主导入的模块必须通过加载校验");
        let entry = loaded
            .exports
            .iter()
            .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with("main"))
            .expect("导出表必须包含 main");
        let mut executor = Executor::new();
        let result = executor.run(&loaded, entry.function_index as usize, Vec::new()).expect("执行");
        assert_eq!(result, Value::I64(20), "and→or→xor→shl→shr(掩码) 必须得到 20");
    }

    #[test]
    fn i64_bitwise_ovm_without_provider_fails_structurally() {
        let groups = vec![
            CompilerSourceGroup::single_source("std", "std", STD_PRIMITIVES),
            CompilerSourceGroup::single_source("core", "core", CONTRACTS).with_direct_dependencies(vec!["std".into()]),
            CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["std".into(), "core".into()]),
        ];
        let error = lower_ovm(&groups, &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
        assert!(error.contains("demo") && error.contains("i64"), "错误必须指向未绑定 provider 的合同: {error}");
    }
}
