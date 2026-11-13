//! WASM GC 数组 operator provider 的端到端合同。
//!
//! 锁定事实：`[wasm_operator("array.*")]` 冻结绑定沿 Compiler →
//! `TargetExecutionPlan` → WASM backend 降低为 wasm GC 数组指令
//! （`array.new_default` / `array.get` / `array.set` / `array.len` 与共享的
//! `ref.cast` 序列），arraytype 索引由 RepresentationPlan 注册表按元素类型
//! 解析；operator 全部内联，不得留下 host 导入。声明签名与冻结计划语义形状
//! 不符、或助记符未知，都必须结构化失败。

use acorn_wasm::WasmBinaryModule;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan, testing::lower_fragment_to_wasm_mir_module};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

/// wasm GC 前缀字节（`0xFB`）+ 子操作码：`array.new_default` / `array.get` /
/// `array.set` / `array.len`；`i32.add` 叶子操作码 `0x6A`。
const ARRAY_NEW_DEFAULT: [u8; 2] = [0xFB, 0x07];
const ARRAY_GET: [u8; 2] = [0xFB, 0x0B];
const ARRAY_SET: [u8; 2] = [0xFB, 0x0E];
const ARRAY_LEN: [u8; 2] = [0xFB, 0x0F];
const I32_ADD: u8 = 0x6A;

const CORE_PRIMITIVE_SOURCE: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }"#;

/// 数组 operator 直接由合同携带：数值加法走数值冻结表，数组操作走数组冻结表。
const ARRAY_OPERATOR_SOURCE: &str = r#"
imply i32 {
    infix `+`(self, rhs: Self): Self {
        i32_add(self, rhs)
    }
}

[host_contract]
[wasm_operator("i32.add")]
micro i32_add(a: i32, b: i32): i32

[host_contract]
[wasm_operator("array.new")]
micro array_new(length: i32): [i32]

[host_contract]
[wasm_operator("array.set")]
micro array_set(xs: [i32], index: i32, value: i32): unit

[host_contract]
[wasm_operator("array.get")]
micro array_get(xs: [i32], index: i32): i32

[host_contract]
[wasm_operator("array.length")]
micro array_length(xs: [i32]): i32

[main]
micro main() -> i32 {
    let xs = array_new(3)
    array_set(xs, 1, 42)
    return array_length(xs) + array_get(xs, 1)
}
"#;

/// 沿 Compiler 产出的目标私有计划降低到 wasm 模块，不重放调用边。
fn try_lower_source_groups(groups: &[CompilerSourceGroup]) -> Result<WasmBinaryModule, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, &[], &[], "native")
        .expect("源码闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::testing::lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main")
        .map_err(|error| error.to_string())
}

fn array_operator_groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::single_source("app", "demo", ARRAY_OPERATOR_SOURCE).as_entry().with_direct_dependencies(vec!["core".into()]),
    ]
}

fn code_section_bytes(module: &WasmBinaryModule) -> &[u8] {
    &module.sections.iter().find(|section| section.id == 10).expect("wasm 模块必须有 code 段").bytes
}

fn import_section_bytes(module: &WasmBinaryModule) -> &[u8] {
    module.sections.iter().find(|section| section.id == 2).map(|section| section.bytes.as_slice()).unwrap_or(&[])
}

#[test]
fn wasm_array_operators_lower_to_gc_array_instructions_without_host_import() {
    let module = try_lower_source_groups(&array_operator_groups()).expect("数组 operator 链必须完成降低");
    let code = code_section_bytes(&module);
    for (opcode, label) in
        [(ARRAY_NEW_DEFAULT, "array.new_default"), (ARRAY_SET, "array.set"), (ARRAY_GET, "array.get"), (ARRAY_LEN, "array.len")]
    {
        assert!(
            code.windows(opcode.len()).any(|window| window == opcode),
            "code 段必须含 `{label}` 指令 [{:#04X}, {:#04X}]",
            opcode[0],
            opcode[1]
        );
    }
    assert!(code.contains(&I32_ADD), "数值 operator `i32.add` 必须内联为叶子操作码");
    let imports = import_section_bytes(&module);
    for symbol in ["array.new", "array.set", "array.get", "array.length", "i32.add"] {
        assert!(!imports.windows(symbol.len()).any(|window| window == symbol.as_bytes()), "已内联的 `{symbol}` 不得留下 host 导入");
    }
}

/// 声明签名与冻结计划语义形状不符（`array.get` 返回类型 ≠ 元素类型）必须在
/// 计划构造期结构化失败，不得让后端发射后再改写。
#[test]
fn wasm_array_operator_signature_mismatch_fails_at_plan_construction() {
    let groups = vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }
[primitive("std::primitive::i64")]
structure i64 { }"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            r#"
[host_contract]
[wasm_operator("array.new")]
micro array_new(length: i32): [i32]

[host_contract]
[wasm_operator("array.get")]
micro array_get(xs: [i32], index: i32): i64

[main]
micro main() -> i64 {
    let xs = array_new(3)
    return array_get(xs, 0)
}
"#,
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];
    let message = try_lower_source_groups(&groups).expect_err("`array.get` 返回类型与元素类型不符必须失败");
    assert!(message.contains("array.get"), "{message}");
}

/// 未知数组助记符不得猜默认操作，必须在发射期结构化失败。
#[test]
fn unknown_wasm_array_mnemonic_fails_structurally() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            r#"
[host_contract]
[wasm_operator("array.foo")]
micro array_foo(x: i32): i32

[main]
micro main() -> i32 {
    return array_foo(1)
}
"#,
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ];
    let message = try_lower_source_groups(&groups).expect_err("未知数组助记符必须结构化失败");
    assert!(message.contains("array.foo"), "{message}");
}

const CONTRACTS: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

namespace demo.collection.array;

[host_contract]
micro new<T>(length: i32): [T]

[host_contract]
micro length<T>(array: [T]): i32

[host_contract]
micro get<T>(array: [T], index: i32): T

[host_contract]
micro set<T>(array: [T], index: i32, value: T): unit
"#;

// 形制 1：`[host_provider]` 包装体调用无体 `[wasm_operator]` micro。
// 泛型 provider 的具体代入由合同调用点在单态化播种中铸造，绑定按同签名闭合实例
// 逐对配对重映射；provider 体内的 `[wasm_operator]` 抽象调用随同一固定点闭合。
const PROVIDERS_WRAPPER: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new)]
private micro array_host_new<T>(length: i32): [T] {
    return __array_wasm_new::<T>(length)
}

[host_provider(demo::collection::array::length)]
private micro array_host_length<T>(array: [T]): i32 {
    return __array_wasm_length::<T>(array)
}

[host_provider(demo::collection::array::get)]
private micro array_host_get<T>(array: [T], index: i32): T {
    return __array_wasm_get::<T>(array, index)
}

[host_provider(demo::collection::array::set)]
private micro array_host_set<T>(array: [T], index: i32, value: T): unit {
    __array_wasm_set::<T>(array, index, value)
}

[wasm_operator("array.new"), pure]
private micro __array_wasm_new<T>(length: i32): [T] { }

[wasm_operator("array.length"), pure]
private micro __array_wasm_length<T>(array: [T]): i32 { }

[wasm_operator("array.get"), pure]
private micro __array_wasm_get<T>(array: [T], index: i32): T { }

[wasm_operator("array.set")]
private micro __array_wasm_set<T>(array: [T], index: i32, value: T): unit { }
"#;

// 形制 2：`[wasm_operator]` 直接挂在无体 `[host_provider]` micro 上（无包装体），
// 与 wasm numeric adaptor 既有 provider 形制一致。
const PROVIDERS_DIRECT: &str = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new), wasm_operator("array.new"), pure]
private micro array_host_new<T>(length: i32): [T] { }

[host_provider(demo::collection::array::length), wasm_operator("array.length"), pure]
private micro array_host_length<T>(array: [T]): i32 { }

[host_provider(demo::collection::array::get), wasm_operator("array.get"), pure]
private micro array_host_get<T>(array: [T], index: i32): T { }

[host_provider(demo::collection::array::set), wasm_operator("array.set")]
private micro array_host_set<T>(array: [T], index: i32, value: T): unit { }
"#;

// 应用与合同同命名空间并使用无限定 turbofish：限定路径 + `::<T>` 的解析
// 正由 core 收尾会话修复（`type_lowering.rs`/`resolver.rs`），本测试不依赖该在途改动。
const APP: &str = r#"
namespace demo.collection.array;

micro pick_two() -> i32 {
    let xs: [i32] = new::<i32>(3)
    set(xs, 1, 42)
    return get(xs, 1)
}

micro sized() -> i32 {
    let xs: [i32] = new::<i32>(5)
    return length(xs)
}

[main]
micro main() -> i32 {
    let n = sized()
    return pick_two()
}
"#;

fn provider_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let array_ns = ["demo".to_owned(), "collection".to_owned(), "array".to_owned()];
    let adaptor_ns = ["demo".to_owned(), "adaptor".to_owned(), "collection".to_owned()];
    [("new", "array_host_new"), ("length", "array_host_length"), ("get", "array_host_get"), ("set", "array_host_set")]
        .into_iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &array_ns, contract),
            provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn groups_for(providers: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", CONTRACTS),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

fn lower(
    groups: &[CompilerSourceGroup],
    bindings: &[rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding],
) -> Result<acorn_wasm::WasmBinaryModule, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(groups, bindings, &[], "native")
        .map_err(|error| error.to_string())?;
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    lower_fragment_to_wasm_mir_module(&FragmentSubmission::from_backend_plan(plan), "main").map_err(|error| error.to_string())
}

/// wasm GC arraytype 的类型段条目形式字节（GC MVP `0x5E`）。
const TYPE_FORM_ARRAY: u8 = 0x5E;

fn assert_gc_array_type_registered(module: &acorn_wasm::WasmBinaryModule) {
    let type_section = module.sections.iter().find(|section| section.id == 1).expect("type 段");
    assert!(type_section.bytes.contains(&TYPE_FORM_ARRAY), "元素类型 `[i32]` 的 arraytype 条目必须登记进类型段");
}

#[test]
fn array_contract_wasm_wrapper_provider_form_lowers() {
    let module = lower(&groups_for(PROVIDERS_WRAPPER), &provider_bindings())
        .expect("形制 1（`[host_provider]` 包装体 + `[wasm_operator]` 无体 micro）必须完成降低");
    assert_gc_array_type_registered(&module);
}

#[test]
fn array_contract_wasm_direct_provider_form_lowers() {
    let module = lower(&groups_for(PROVIDERS_DIRECT), &provider_bindings()).expect("形制 2（`[wasm_operator]` 直挂无体 provider）必须完成降低");
    assert_gc_array_type_registered(&module);
}

#[test]
fn array_contract_wasm_without_provider_fails_structurally() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CONTRACTS),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let error = lower(&groups, &[]).expect_err("缺 provider 的合同调用必须结构化失败，不得空体成功");
    assert!(error.contains("demo") && error.contains("array"), "错误必须指向未绑定 provider 的合同: {error}");
}

#[test]
fn array_contract_wasm_unknown_mnemonic_fails_structurally() {
    let providers = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new), wasm_operator("array.push"), pure]
private micro array_host_new<T>(length: i32): [T] { }

[host_provider(demo::collection::array::length), wasm_operator("array.length"), pure]
private micro array_host_length<T>(array: [T]): i32 { }

[host_provider(demo::collection::array::get), wasm_operator("array.get"), pure]
private micro array_host_get<T>(array: [T], index: i32): T { }

[host_provider(demo::collection::array::set), wasm_operator("array.set")]
private micro array_host_set<T>(array: [T], index: i32, value: T): unit { }
"#;
    let error = lower(&groups_for(providers), &provider_bindings()).expect_err("未知数组助记符必须落 `Unfrozen` 并在 emit 结构化失败");
    assert!(error.contains("array.push") && error.contains("可编码"), "错误必须指向未冻结的助记符: {error}");
}

#[test]
fn array_contract_wasm_signature_shape_mismatch_fails_structurally() {
    // `array.length` 的冻结形状要求返回携带 i32 载体；合同声明 i64 返回、provider 与
    // 合同同签名配对成功后，计划构造期必须按声明签名与冻结计划不符结构化失败，
    // 不得让后端发射后再改写。
    let contracts = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

namespace demo.collection.array;

[host_contract]
micro new<T>(length: i32): [T]

[host_contract]
micro length<T>(array: [T]): i64
"#;
    let providers = r#"
namespace demo.adaptor.collection;

[host_provider(demo::collection::array::new), wasm_operator("array.new"), pure]
private micro array_host_new<T>(length: i32): [T] { }

[host_provider(demo::collection::array::length), wasm_operator("array.length"), pure]
private micro array_host_length<T>(array: [T]): i64 { }
"#;
    let app = r#"
namespace demo.collection.array;

[main]
micro main() -> i32 {
    let xs: [i32] = new::<i32>(5)
    let n = length(xs)
    return 0
}
"#;
    let array_ns = ["demo".to_owned(), "collection".to_owned(), "array".to_owned()];
    let adaptor_ns = ["demo".to_owned(), "adaptor".to_owned(), "collection".to_owned()];
    let bindings = [("new", "array_host_new"), ("length", "array_host_length")]
        .into_iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: rusty_valkyrie::compiler::semantic::closure::micro_declaration_key("core", "core.v", &array_ns, contract),
            provider: rusty_valkyrie::compiler::semantic::closure::micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect::<Vec<_>>();
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", contracts),
        CompilerSourceGroup::single_source("adaptor", "adaptor", providers).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", app).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let error = lower(&groups, &bindings).expect_err("声明签名与冻结数组计划不符必须结构化失败");
    assert!(error.contains("array.length") && error.contains("i32"), "错误必须指向返回载体不符: {error}");
}
