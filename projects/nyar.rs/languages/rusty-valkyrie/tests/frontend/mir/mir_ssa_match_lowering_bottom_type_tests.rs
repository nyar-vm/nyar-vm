use rusty_valkyrie::{
    mir::{MirConstant, MirOperand},
    types::hir::ValkyrieType,
};

use rusty_valkyrie::compiler::mir::ssa::arm_contributes_to_match_merge;
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

#[test]
fn only_structured_bottom_excludes_an_arm_from_merge() {
    let result = MirOperand::Constant(MirConstant::Int(1));
    assert!(!arm_contributes_to_match_merge(&result, Some(&ValkyrieType::Void), None));
    let ordinary_named_type = ValkyrieType::Named(rusty_valkyrie::compiler::types::Identifier::new("Never"));
    assert!(arm_contributes_to_match_merge(&result, Some(&ordinary_named_type), None));
}

/// `void`（never）与 `unit` 一样不产生可物化值槽（BPHYS001）。
///
/// 正例：以 `void` 函数为语句与 match 臂值的程序必须完成编译——`void` 调用不占
/// SSA 值槽，未捕获 `raise` 的 `Resume = !` 恢复块不声明形参。
/// 反例：函数体只剩 `void` 调用却声明返回非 `!` 类型时，必须保持结构化失败，
/// 证明该豁免不是把 `void` 静默当成任意类型。
const PANIC_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i64")]
structure i64 { }

namespace std.types;

micro panic<T>(message: T): void {
    raise message
}

micro unreachable(): void {
    std::types::panic("unreachable")
}
"#;

fn compile_with_panic_core(app: &str) -> Result<(), String> {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PANIC_CORE),
        CompilerSourceGroup::single_source("app", "demo", app).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[test]
fn void_call_statement_does_not_occupy_value_slots() {
    let app = r#"
namespace demo;

[export]
micro probe(flag: bool) -> bool {
    if flag {
        std::types::panic("boom")
    }
    if flag {
        std::types::unreachable()
    }
    return flag
}
"#;
    compile_with_panic_core(app).expect("语句位置的 `void` 调用不得占用物理值槽");
}

#[test]
fn basic_carrier_raise_keeps_one_resume_parameter() {
    let app = r#"
namespace demo;

using std.primitive::{i64}

[export]
micro probe(flag: bool) -> i64 {
    if flag {
        raise 42
    }
    return 7
}
"#;
    compile_with_panic_core(app).expect("基本类型载体的 raise 保留单形参恢复块");
}

#[test]
fn void_call_as_return_value_of_non_never_signature_still_fails() {
    let app = r#"
namespace demo;

[export]
micro probe() -> bool {
    return std::types::unreachable()
}
"#;
    let error = compile_with_panic_core(app).expect_err("`void` 结果不得冒充 `bool` 返回值");
    assert!(error.contains("SMIR007") || error.contains("return"), "必须保持返回合同失败，实际：{error}");
}
