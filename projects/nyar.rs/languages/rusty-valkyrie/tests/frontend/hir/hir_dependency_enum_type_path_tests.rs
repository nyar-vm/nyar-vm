//! 依赖组导出的枚举在限定类型名位置的解析合同。
//!
//! `std::operator::Ordering` 这类「模块路径 + 枚举名」必须按完整声明身份命中可见
//! 名义类型。此前依赖导出的枚举只以空 namespace 登记，消费者在参数、返回或字段
//! 类型位置引用时得到 `unknown type name`（`eliminate-maximum-number-of-monsters`
//! 的真实阻塞）。本用例锁定「模块级枚举按导出模块 namespace 可见」。

use rusty_valkyrie::{
    CompilerSourceFile, CompilerSourceGroup, HirError, ValkyrieCompiler, hir::lowering::DEFAULT_COMPILE_ARCH,
    types::hir::HirModule,
};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

type usize = u64;

[primitive("std::primitive::bool")]
structure bool { }
"#;

/// 模块级枚举：`enums` 声明没有独立 namespace 字段，声明域就是导出模块。
const OPERATOR: &str = r#"
namespace std.operator;

enums Ordering {
    Less
    Equal
    Greater
}
"#;

/// 消费者只在**类型位置**引用依赖枚举，不产生任何调用或名字引用。
const CONSUMER: &str = r#"
namespace app;

[export] micro run(state: std::operator::Ordering) -> bool {
    return true
}
"#;

fn compile(consumer: &str) -> Result<Vec<HirModule>, HirError> {
    let groups = [
        CompilerSourceGroup::from_source_files(
            "std",
            "std",
            vec![
                CompilerSourceFile { path: "primitives.v".into(), source: PRIMITIVES.into() },
                CompilerSourceFile { path: "operator.v".into(), source: OPERATOR.into() },
            ],
            Vec::new(),
        ),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]),
    ];
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
}

#[test]
fn dependency_enum_resolves_as_qualified_type_name() {
    compile(CONSUMER).expect("依赖组导出的 `std::operator::Ordering` 必须能按限定路径解析");
}

#[test]
fn dependency_enum_short_name_resolves_like_exported_structs() {
    // 依赖导出的短名可见性沿用既有合同（与 `HirStruct` 一致），本用例只是锁定没有回归。
    let consumer = r#"
namespace app;

[export] micro run(state: Ordering) -> bool {
    return true
}
"#;
    compile(consumer).expect("依赖枚举的短名与导出结构体同口径可见");
}

#[test]
fn dependency_enum_rejects_wrong_namespace_path() {
    // 限定路径按完整声明身份匹配：namespace 不符必须结构化失败，不得按末段放行。
    let consumer = r#"
namespace app;

[export] micro run(state: std::text::Ordering) -> bool {
    return true
}
"#;
    let error = compile(consumer).expect_err("错误 namespace 的限定路径不得命中依赖枚举");
    assert!(error.to_string().contains("unknown type name"), "{error}");
}
