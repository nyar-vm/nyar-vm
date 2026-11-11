//! 回归：host provider 体迁入消费模块后，其 `SumNew` 必须使用消费模块冻结后的
//! 全局 sum/variant 身份，而不是 provider 原模块 lowering 时的本地身份。
//!
//! 复现 `leetcode.v` `config-table-report` 的 SMIR006：provider 体构造 `Option` 的
//! `None`/`Some`，迁入消费模块后本地 `NominalInstanceId`/`VariantId` 未按来源模块
//! 的 aggregate remap 归一，`sum construction references an undeclared variant`。

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::{
        emitter::pipeline::CompilerHostProviderBinding,
        hir::lowering::DEFAULT_COMPILE_ARCH,
        semantic::closure::micro_declaration_key,
    },
};

use crate::primitives::groups_with_primitives;

fn sources() -> Vec<CompilerSourceGroup> {
    // `core`：声明 `Option<T>` 与 [`host_contract`] `index_of`。
    let core = r#"
namespace demo.types;

[tag(OptionKind)]
unite Option<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}

namespace demo.text;

using demo.types

class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text;

[host_contract]
micro index_of(value: utf8, other: utf8, found: bool): Option<usize>
"#;
    // `adaptor`：provider 体在自身模块 lowering，按本地身份构造 None/Some。
    let adaptor = r#"
namespace demo.adaptor;

using demo.types

[host_provider(demo::text::index_of)]
micro host_index_of(value: demo.text.utf8, other: demo.text.utf8, found: bool): Option<usize> {
    if found {
        return Some(0)
    }
    return None
}
"#;
    // `entry`：调用合同，驱动 provider 迁移到消费模块。消费模块多声明一个聚合，
    // 使 `Option` 的本地 `NominalInstanceId` 与 provider 原模块不同，才能暴露
    // 「迁入副本按消费模块 remap 重映射」的错位。
    let entry = r#"
namespace demo.app;

using demo.types
using demo.text

class ProbeRecord {
    tag: i64
}

[export(name: "probeEntry")]
micro probe_entry(value: utf8, other: utf8, found: bool) -> i64 {
    let record = ProbeRecord { tag: 1 }
    let slot: Option<usize> = index_of(value, other, found)
    return record.tag
}
"#;
    groups_with_primitives(vec![
        CompilerSourceGroup::single_source("core", "core", core),
        CompilerSourceGroup::single_source("adaptor", "adaptor", adaptor)
            .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("entry", "entry", entry)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ])
}

fn bindings() -> Vec<CompilerHostProviderBinding> {
    vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("core", "core.v", &["demo".into(), "text".into()], "index_of"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &["demo".into(), "adaptor".into()], "host_index_of"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }]
}

#[test]
fn provider_sum_construction_uses_consumer_aggregate_identity() {
    let result = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&sources(), &bindings(), &[], DEFAULT_COMPILE_ARCH)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"));
    assert!(result.is_ok(), "provider 体迁入消费模块后 sum 身份必须归一，实际: {result:?}");
}
