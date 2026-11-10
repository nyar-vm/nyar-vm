//! 枚举接收者上的 `infix ==` 解析合同（复现 `enum-equality` 场景）。
//!
//! 场景形态：纯标记 `enums` 声明本地类型，`let` 以裸变体绑定枚举值，两个枚举值
//! 直接写 `==`。std 侧合同明确：`enums` 不自带 `==`，相等比较必须由 `imply X: Equal`
//! 显式声明（见 `valkyrie.v` `std/operator/comparable.v` 手写的
//! `imply Ordering: std::operator::Equal`）。本文件冻结两个方向：
//! - 枚举没有 Equal impl：调用点必须 fail-closed 为 `UnresolvedCall`，
//!   失败类别是「同名候选存在但接收者类型不匹配」（场景当前失败形态）；
//! - 显式声明 `imply Phase: Equal`：impl 成员进入候选池并按 `impl_target`
//!   与枚举接收者匹配，解析成功。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    types::hir::{HirCallResolutionFailureKind, HirExprKind},
};

use crate::primitives::SourceFixtureCompile;

const EQUAL_TRAIT: &str = "trait Equal { infix `==`(self, other: Self): bool infix `!=`(self, other: Self): bool } ";

const PHASE_ENUM: &str = "enums Phase { Alpha Beta } ";

const PROBE: &str = "micro same_enum_equal(): bool { let left: Phase = Alpha let right: Phase = Alpha return left == right } ";

/// 场景复现：纯标记枚举直接写 `==`、无 Equal impl 时，`infix ==` 必须 fail-closed，
/// 失败类别为同名候选存在但无一匹配枚举接收者（`NoViableOverload`）。
#[test]
fn enum_receiver_infix_eq_without_equal_impl_fails_closed() {
    let error = ValkyrieCompiler::default()
        .compile_fixture_source(&format!("{EQUAL_TRAIT}{PHASE_ENUM}{PROBE}"))
        .expect_err("`enums` 不自带 `==`：无 Equal impl 时枚举接收者的 `infix ==` 必须 fail-closed");
    match &error {
        HirError::UnresolvedCall { selector, reason, .. } => {
            assert_eq!(selector, "infix ==", "got {error:?}");
            assert!(
                matches!(reason, HirCallResolutionFailureKind::NoViableOverload { .. }),
                "枚举接收者的失败必须是「同名候选无一匹配」，got {reason:?}"
            );
        }
        other => panic!("expected UnresolvedCall, got {other:?}"),
    }
}

/// 声明侧修复路径必须可用：场景补 `imply Phase: Equal` 后，impl 成员成为候选并解析。
#[test]
fn enum_receiver_infix_eq_resolves_through_declared_equal_impl() {
    let hir = ValkyrieCompiler::default()
        .compile_fixture_source(&format!(
            "{EQUAL_TRAIT}{PHASE_ENUM}\
             imply Phase: Equal {{ infix `==`(self, other: Self): bool {{ return true }} \
             infix `!=`(self, other: Self): bool {{ return false }} }}\
             {PROBE}"
        ))
        .expect("声明 `imply Phase: Equal` 后，枚举接收者的 `infix ==` 必须解析");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "same_enum_equal").expect("same_enum_equal");
    let expression = function.body.expr.as_deref().expect("return 表达式");
    let HirExprKind::Return(Some(value)) = &expression.kind else {
        panic!("{expression:?}")
    };
    let HirExprKind::Call { resolved, .. } = &value.kind else {
        panic!("{value:?}")
    };
    let resolved = resolved.as_ref().expect("`infix ==` 必须解析");
    assert_eq!(resolved.symbol.to_string(), "Phase.infix ==");
}
