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
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::{emitter::pipeline::CompilerHostProviderBinding, semantic::closure::micro_declaration_key},
};

const CORE_PRIMITIVE_SOURCE: &str = r#"[primitive("std::primitive::i32")]
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
const APP_SOURCE: &str = r#"class Utf8Text {
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
        contract: micro_declaration_key("app", "demo.v", &[], "length"),
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
fn lower() -> Result<odin_bytecode::OdinModuleData, String> {
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

fn run_main(module: &odin_bytecode::OdinModuleData) -> Value {
    let entry = module
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|name| name.ends_with("main"))
        .unwrap_or_else(|| panic!("必须导出 main 入口: {:?}", module.exports));
    let bytes = odin_bytecode::encode_module(module);
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
