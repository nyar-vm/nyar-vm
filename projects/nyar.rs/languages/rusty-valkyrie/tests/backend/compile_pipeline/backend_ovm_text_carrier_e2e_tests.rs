//! OVM 文本载体边界的可证伪回归：内部调用的 `utf8` 实参必须按目标计划冻结的
//! layout 物化成托管对象，raw 宿主串不得直接进入 `FieldGet` 字段访问。
//!
//! 夹具故意只走「方法读自身 `class` 布局字段（`_repr`）」这一条最小路径：
//! 没有本次修复时，`main` 里的 `"abc"` 仍是 `Value::String`，直接传给
//! `repr_byte_length` 的 `self`，`self._repr` 的 `FieldGet` 会在 odin-vm 运行时
//! 以 `type mismatch: expected object, got string` 失败；只有先经
//! `utf8.to_bytes` → `ObjectNew` → `FieldSet 0` 物化才能读到 3。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_vm::Value;
use rusty_valkyrie::{
    CompilerSourceFile, CompilerSourceGroup, ValkyrieCompiler,
    compiler::{emitter::pipeline::CompilerHostProviderBinding, semantic::closure::micro_declaration_key},
};

const CORE_PRIMITIVE_SOURCE: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::usize")]
structure usize { }"#;

/// 数组长度 provider：无体，目标事实由 `[ovm("array.length")]` 宿主导入携带。
const ARRAY_PROVIDER_SOURCE: &str = r#"[host_provider(demo::array_length)]
[ovm("array.length")]
micro ovm_array_length<T>(xs: [T]): usize"#;

/// 最小文本载体夹具：方法读取自身 `class` 布局字段 `_repr`。
///
/// `Utf8Text` 必须声明在 `std.text`：canonical 只用这份显式命名空间事实把
/// 文本载体登记进 `text_encoding_types`，字符串常量与名义类型才被判等价。
const APP_SOURCE: &str = r#"namespace std.text;

[primitive("std::text::utf8")]
class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text

[host_contract]
micro length<T>(xs: [T]): usize

imply Utf8Text {
    micro repr_byte_length(self) -> usize {
        return length(self._repr)
    }
}

[main]
micro main() -> usize {
    let text: utf8 = "abc"
    return text.repr_byte_length()
}"#;

fn bindings() -> Vec<CompilerHostProviderBinding> {
    vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &["std".to_owned(), "text".to_owned()], "length"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "ovm_array_length"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }]
}

fn source_groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::single_source("adaptor", "adaptor", ARRAY_PROVIDER_SOURCE).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", APP_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

/// 沿 Compiler 产出的目标私有计划降低到 `.odin`，不重放调用边。
fn lower() -> Result<acorn_odin::OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&source_groups(), &bindings(), &[], "native")
        .map_err(|error| error.to_string())?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn run_main(module: &acorn_odin::OdinModuleData) -> Value {
    let entry = module
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|name| name.ends_with("main"))
        .unwrap_or_else(|| panic!("必须导出 main 入口: {:?}", module.exports));
    let bytes = acorn_odin::encode_module(module);
    let mut vm = odin_vm::OdinVm::new();
    let loaded = vm.load(&bytes).expect("编码后的模块必须可加载");
    vm.run(&loaded, &entry, Vec::new()).expect("main 必须在内置 OVM 运行时执行成功")
}

/// 不带本次修复时该用例必然失败：`text` 是 raw 串，`repr_byte_length` 的
/// `self._repr` 会以 `expected object, got string` 拒绝字段访问。
#[test]
fn utf8_receiver_is_materialized_before_field_access() {
    let module = lower().expect("文本载体边界必须完成降低");
    assert!(
        module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == "utf8.to_bytes"),
        "进入内部 `utf8` 形参前必须走 `utf8.to_bytes` 物化桥: {:?}",
        module.imports
    );
    assert_eq!(run_main(&module), Value::I32(3), "`self._repr` 必须读到托管文本对象的 `[u8]` 槽长度");
}

/// 值位置 `if` 合并 fixture：一臂是 raw 字面量 `"abc"`，另一臂是托管文本对象
/// （`from_bytes`）。SSA 合并块参数按目标计划冻结为托管文本布局；走 raw 臂时，
/// 块参数入边必须物化，否则下游 `self._repr` 以 `expected object, got string` 崩溃。
const MERGE_APP_SOURCE: &str = r#"namespace std.text;

[primitive("std::text::utf8")]
class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text

[host_contract]
micro length<T>(xs: [T]): usize

imply Utf8Text {
    micro from_bytes(bytes: [u8]) -> Self {
        return Self { _repr: bytes }
    }

    micro repr_byte_length(self) -> usize {
        return length(self._repr)
    }
}

[main]
micro main(flag: bool) -> usize {
    let text: utf8 = if flag { "abc" } else { Utf8Text::from_bytes([104, 105]) }
    return text.repr_byte_length()
}"#;

fn merge_bindings() -> Vec<CompilerHostProviderBinding> {
    vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &["std".to_owned(), "text".to_owned()], "length"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "ovm_array_length"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }]
}

fn merge_source_groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::single_source("adaptor", "adaptor", ARRAY_PROVIDER_SOURCE).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", MERGE_APP_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ]
}

fn lower_merge() -> Result<acorn_odin::OdinModuleData, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&merge_source_groups(), &merge_bindings(), &[], "native")
        .map_err(|error| error.to_string())?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

fn run_main_with_args(module: &acorn_odin::OdinModuleData, args: Vec<Value>) -> Value {
    let entry = module
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|name| name.ends_with("main"))
        .unwrap_or_else(|| panic!("必须导出 main 入口: {:?}", module.exports));
    let bytes = acorn_odin::encode_module(module);
    let mut vm = odin_vm::OdinVm::new();
    let loaded = vm.load(&bytes).expect("编码后的模块必须可加载");
    vm.run(&loaded, &entry, args).expect("main 必须在内置 OVM 运行时执行成功")
}

/// 不带块参数物化时该用例必然失败：raw `"abc"` 臂流入托管合并块参数，
/// `text.repr_byte_length()` 的 `self._repr` 以 `expected object, got string` 拒绝。
#[test]
fn value_if_merge_materializes_raw_text_into_block_parameter() {
    let module = lower_merge().expect("值合并文本边界必须完成降低");
    assert!(
        module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == "utf8.to_bytes"),
        "raw 文本臂进入托管合并块参数前必须走 `utf8.to_bytes` 物化桥: {:?}",
        module.imports
    );
    assert_eq!(run_main_with_args(&module, vec![Value::Bool(true)]), Value::I32(3), "raw `\"abc\"` 臂必须物化为托管文本对象后读到 3 字节");
    // 托管臂（`from_bytes([104,105])`）走同一块参数槽，读到 2 字节。
    assert_eq!(run_main_with_args(&module, vec![Value::Bool(false)]), Value::I32(2), "托管文本臂必须按同一合并块参数槽读到 2 字节");
}

/// 与 std 同形的泛型 panic：文本实参经泛型单态化成为 `utf8` 形参。
const PANIC_DECLARATION_SOURCE: &str = r#"namespace std.types;

micro panic<T>(message: T): void {
    raise message
}"#;

/// 纯 raw 片段：只把 `utf8` 字面量直传给泛型 `panic`，不声明任何 `std.text`
/// 文本载体聚合。正式表示计划据此把程序级文本表示事实声明为 raw——这正是
/// bitset 等工程里 `std::types::panic("...")` 的冻结事实，不能当成「缺布局」失败。
const PANIC_APP_SOURCE: &str = r#"namespace demo.app;

[main]
micro main() -> void {
    std::types::panic("boom")
}"#;

fn panic_source_groups() -> Vec<CompilerSourceGroup> {
    let files = vec![
        CompilerSourceFile {
            path: "types.v".into(),
            source: PANIC_DECLARATION_SOURCE.into(),
        },
        CompilerSourceFile {
            path: "app.v".into(),
            source: PANIC_APP_SOURCE.into(),
        },
    ];
    vec![
        CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE),
        CompilerSourceGroup::from_source_files("app", "app", files, Vec::new())
            .as_entry()
            .with_direct_dependencies(vec!["core".into()]),
    ]
}

fn compile_panic() -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default().compile_source_groups_to_program(&panic_source_groups()).map_err(|error| error.to_string())
}

fn lower_panic() -> Result<acorn_odin::OdinModuleData, String> {
    let program = compile_panic()?;
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())
}

/// 纯 raw 片段里的 `std::types::panic` 文本实参：正式表示计划在没有声明任何
/// `std.text` 载体时显式声明程序级 raw，目标计划从这份事实冻结 raw 槽，
/// 降低必须成功；布局注册表为空不再参与判定。
#[test]
fn panic_text_argument_follows_frozen_slot_carrier() {
    let program = compile_panic().expect("纯 raw 片段必须完成编译");
    assert_eq!(
        program.representation().text_representation,
        Some(nyar_types::layout_choice::TextRepresentation::Raw),
        "未声明任何文本载体的片段必须由正式表示计划显式声明 raw"
    );
    lower_panic().expect("纯 raw 片段中的 `panic` 文本实参必须按冻结 raw 槽完成降低");
}

/// 反向合同：同一份已冻结计划若没有程序级文本表示事实，`utf8` 槽是 missing
/// 事实，必须在 `CompiledProgram` 构造期结构化拒收，不得把「布局表没有托管布局」
/// 解释为 raw，也不得留到目标层。
#[test]
fn undeclared_program_text_representation_is_rejected() {
    let program = compile_panic().expect("纯 raw 片段必须完成编译");
    assert_eq!(
        program.representation().text_representation,
        Some(nyar_types::layout_choice::TextRepresentation::Raw),
        "未声明任何文本载体的片段必须由正式表示计划显式声明 raw"
    );
    // 刻意清掉程序级事实：raw 必须来自声明，不能由「没有托管布局」反推。
    let mut representation = program.representation().clone();
    representation.text_representation = None;
    let error = nyar_types::CompiledProgram::new(program.canonical().clone(), representation)
        .expect_err("没有程序级文本表示事实的 `utf8` 程序必须结构化拒收");
    assert_eq!(
        error,
        nyar_types::CompiledProgramError::TextRepresentationMismatch {
            expected: Some(nyar_types::layout_choice::TextRepresentation::Raw),
            actual: None,
        }
    );
    assert!(error.to_string().contains("TextRepresentationMismatch"), "错误必须点名构造期合同，实际: {error}");
}
