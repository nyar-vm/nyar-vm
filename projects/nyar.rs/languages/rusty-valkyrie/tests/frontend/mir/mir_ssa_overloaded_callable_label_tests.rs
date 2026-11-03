//! 同类型重载方法的 ABI 诊断标签合同。
//!
//! `Utf8Builder` 形状：`imply` 内的两个 `new` 是非泛型 impl 方法，
//! `display_symbol` 都是 `Utf8Builder.new`。标签表以 `ItemInstanceId` 区分身份，
//! 不把合法重载当成 lowering 错误；同一实例出现两个标签仍是真实身份矛盾，
//! 必须进入结构化诊断并让构建失败闭合。

use std::collections::BTreeSet;

use nyar_types::ItemInstanceId;
use rusty_valkyrie::{
    compiler::{
        hir::ValkyrieCompiler,
        mir::validation::{MirValidationError, validate_module},
        types::SourceID,
    },
    mir::ssa::{MirDiagnostic, MirLowerer, MirModule},
};

const OVERLOADED_BUILDER_SOURCE: &str = r#"
namespace std.text;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u64")]
structure u64 { }

type usize = u64;

class Utf8Builder {
    payload: i32
}

imply Utf8Builder {
    micro new(capacity: usize) -> Self {
        return Utf8Builder { payload: 0 }
    }

    micro new(seed: i32) -> Self {
        return Utf8Builder { payload: seed }
    }
}
"#;

/// 复刻 `Utf8Builder` 的重载形状并 lower 到 Semantic MIR。
fn lower_overloaded_builder() -> MirModule {
    let hir =
        ValkyrieCompiler::new(SourceID { version_id: 9410 }).compile_source(OVERLOADED_BUILDER_SOURCE).expect("重载 impl 源码必须完成 HIR");
    MirLowerer::lower_module_semantic(&hir)
}

/// 收集 `Utf8Builder.new` 两个重载实例，按 `ItemInstanceId` 排序。
fn overloaded_new_instances(mir: &MirModule) -> Vec<ItemInstanceId> {
    let mut instances = mir
        .functions
        .iter()
        .filter(|function| function.display_symbol == "Utf8Builder.new")
        .filter_map(|function| function.instance)
        .collect::<Vec<_>>();
    instances.sort();
    instances
}

#[test]
fn overloaded_impl_methods_get_unique_labels_without_panicking() {
    let mir = lower_overloaded_builder();
    assert!(mir.diagnostics.is_empty(), "合法重载不得产生 lowering 诊断: {:?}", mir.diagnostics);

    let instances = overloaded_new_instances(&mir);
    assert_eq!(instances.len(), 2, "两个 `new` 重载都必须进入 MIR");
    assert_eq!(instances.iter().copied().collect::<BTreeSet<_>>().len(), 2, "两个重载的 `ItemInstanceId` 身份不得被合并: {instances:?}");

    let labels = instances.iter().map(|instance| mir.callable_symbols.get(instance).expect("每个重载实例都必须有诊断标签")).collect::<Vec<_>>();
    assert_eq!(labels.iter().copied().collect::<BTreeSet<_>>().len(), 2, "两个重载实例的标签必须唯一: {labels:?}");
    assert!(labels.iter().any(|label| label.as_str() == "Utf8Builder.new"), "先登记项必须保持与源码符号逐字一致: {labels:?}");
    assert!(labels.iter().any(|label| label.as_str() != "Utf8Builder.new"), "冲突项必须追加实例身份判据: {labels:?}");

    // 唯一化只发生在冲突项；全表仍需 instance 与标签一一对应。
    assert_eq!(
        mir.callable_symbols.values().collect::<BTreeSet<_>>().len(),
        mir.callable_symbols.len(),
        "callable 标签表必须 instance 与标签一一对应"
    );

    assert!(validate_module(&mir).is_ok(), "合法重载 lowering 必须通过 MIR 校验");
}

#[test]
fn one_instance_with_two_labels_still_fails_closed() {
    let mut mir = lower_overloaded_builder();
    let instance = overloaded_new_instances(&mir)[0];
    mir.diagnostics.push(MirDiagnostic::ConflictingCallableLabel {
        instance,
        existing: "Utf8Builder.new".to_owned(),
        incoming: format!("Utf8Builder.new#{}", instance.index()),
    });

    let error = validate_module(&mir).expect_err("同一实例两个标签必须失败闭合");
    assert!(matches!(error, MirValidationError::DiagnosticRejected { .. }), "必须是结构化诊断拒绝，实际: {error}");
    let text = error.to_string();
    assert!(text.contains("Utf8Builder.new"), "诊断必须携带冲突标签: {text}");
    assert!(text.contains(&instance.to_string()), "诊断必须携带实例身份: {text}");
}
