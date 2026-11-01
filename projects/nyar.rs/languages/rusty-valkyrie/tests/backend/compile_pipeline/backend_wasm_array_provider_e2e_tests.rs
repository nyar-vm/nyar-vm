//! WASM GC 数组 operator provider 的端到端合同。
//!
//! 锁定事实：`[wasm_operator("array.*")]` 冻结绑定沿 Compiler →
//! `TargetExecutionPlan` → WASM backend 降低为 wasm GC 数组指令
//! （`array.new_default` / `array.get` / `array.set` / `array.len` 与共享的
//! `ref.cast` 序列），arraytype 索引由 RepresentationPlan 注册表按元素类型
//! 解析；operator 全部内联，不得留下 host 导入。声明签名与冻结计划语义形状
//! 不符、或助记符未知，都必须结构化失败。

use acorn_wasm::WasmBinaryModule;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

/// wasm GC 前缀字节（`0xFB`）+ 子操作码：`array.new_default` / `array.get` /
/// `array.set` / `array.len`；`i32.add` 叶子操作码 `0x6A`。
const ARRAY_NEW_DEFAULT: [u8; 2] = [0xFB, 0x07];
const ARRAY_GET: [u8; 2] = [0xFB, 0x0B];
const ARRAY_SET: [u8; 2] = [0xFB, 0x0E];
const ARRAY_LEN: [u8; 2] = [0xFB, 0x0F];
const I32_ADD: u8 = 0x6A;

const CORE_PRIMITIVE_SOURCE: &str = r#"[primitive("std::primitive::i32")]
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
        .compile_source_groups_to_program_with_host_bindings(groups, &[], "native")
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
            r#"[primitive("std::primitive::i32")]
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
