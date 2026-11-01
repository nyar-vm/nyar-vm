//! 具体类型接收者上的运算符成员调用必须经该类型已登记的 trait impl 成员解析。
//!
//! `std::primitive` 把 `==` / `!=` 声明在 `trait Equal` 上，并以 `imply i64: Equal`
//! 提供实现；调用位点（`x != y`）降低为普通成员名 `infix !=`。本文件冻结：
//! - 具体类型接收者的 `infix ==` / `infix !=` 解析成功且指向 impl 成员；
//! - 没有 impl 时保持 fail-closed（`UnresolvedCall`）；
//! - 普通（非运算符）方法经 impl 成员解析，trait 声明成员（`trait_owner`）不得被具体接收者直接解析。

use rusty_valkyrie::{HirError, ValkyrieCompiler};

const PRIMITIVES: &str = "[primitive(\"std::primitive::i64\")] structure i64 { } \
    [primitive(\"std::primitive::usize\")] structure usize { } ";

const EQUAL_TRAIT: &str = "trait Equal { infix `==`(self, other: Self): bool infix `!=`(self, other: Self): bool } ";

const I64_IMPL: &str = "imply i64: Equal { \
    infix `==`(self, other: Self): bool { return true } \
    infix `!=`(self, other: Self): bool { return false } \
} ";

const USIZE_IMPL: &str = "imply usize: Equal { \
    infix `==`(self, other: Self): bool { return true } \
    infix `!=`(self, other: Self): bool { return false } \
} ";

fn probe(source: &str) -> String {
    format!("{PRIMITIVES}{EQUAL_TRAIT}{I64_IMPL}{USIZE_IMPL}{source}")
}

#[test]
fn concrete_receiver_infix_ne_resolves_through_trait_impl() {
    let hir = ValkyrieCompiler::default()
        .compile_source(&probe("micro probe(x: i64, y: i64): bool { return x != y }"))
        .expect("具体类型接收者的 `infix !=` 必须经 `imply i64: Equal` 解析");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("probe");
    let expression = function.body.expr.as_deref().expect("return 表达式");
    let rusty_valkyrie::types::hir::HirExprKind::Return(Some(value)) = &expression.kind
    else {
        panic!("{expression:?}")
    };
    let rusty_valkyrie::types::hir::HirExprKind::Call { resolved, .. } = &value.kind
    else {
        panic!("{value:?}")
    };
    let resolved = resolved.as_ref().expect("infix != 必须解析");
    assert_eq!(resolved.symbol.to_string(), "i64.infix !=", "必须绑定到 i64 的 impl 成员");
}

#[test]
fn concrete_receiver_infix_eq_resolves_through_trait_impl() {
    let hir = ValkyrieCompiler::default()
        .compile_source(&probe("micro probe(x: i64, y: i64): bool { return x == y }"))
        .expect("具体类型接收者的 `infix ==` 必须经 `imply i64: Equal` 解析");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("probe");
    let expression = function.body.expr.as_deref().expect("return 表达式");
    let rusty_valkyrie::types::hir::HirExprKind::Return(Some(value)) = &expression.kind
    else {
        panic!("{expression:?}")
    };
    let rusty_valkyrie::types::hir::HirExprKind::Call { resolved, .. } = &value.kind
    else {
        panic!("{value:?}")
    };
    let resolved = resolved.as_ref().expect("infix == 必须解析");
    assert_eq!(resolved.symbol.to_string(), "i64.infix ==");
}

#[test]
fn missing_impl_stays_fail_closed() {
    let error = ValkyrieCompiler::default()
        .compile_source(&format!("{PRIMITIVES}{EQUAL_TRAIT}micro probe(x: i64, y: i64): bool {{ return x != y }}"))
        .expect_err("没有 `imply i64: Equal` 时 `infix !=` 不得解析");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "got {error:?}");
}

/// 具体类型接收者的普通（非运算符）方法经已登记 impl 成员解析，与运算符成员同一合同。
#[test]
fn concrete_receiver_plain_method_resolves_through_trait_impl() {
    let hir = ValkyrieCompiler::default()
        .compile_source(&format!(
            "{PRIMITIVES}trait Spin {{ micro spin(self): bool }} \
             imply i64: Spin {{ micro spin(self): bool {{ return true }} }} \
             micro probe(x: i64): bool {{ return x.spin() }}"
        ))
        .expect("具体类型接收者的普通方法必须经 `imply i64: Spin` 解析");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "probe").expect("probe");
    let expression = function.body.expr.as_deref().expect("return 表达式");
    let rusty_valkyrie::types::hir::HirExprKind::Return(Some(value)) = &expression.kind
    else {
        panic!("{expression:?}")
    };
    let rusty_valkyrie::types::hir::HirExprKind::Call { resolved, .. } = &value.kind
    else {
        panic!("{value:?}")
    };
    let resolved = resolved.as_ref().expect("spin 必须解析");
    assert_eq!(resolved.symbol.to_string(), "i64.spin");
}

/// trait 声明成员（无实例体）不得被具体类型接收者直接解析；`trait_owner` 过滤保持 fail-closed。
#[test]
fn trait_declaration_member_not_callable_on_concrete_receiver() {
    let error = ValkyrieCompiler::default()
        .compile_source(&format!(
            "{PRIMITIVES}trait Spin {{ micro spin(self): bool }} \
             micro probe(x: i64): bool {{ return x.spin() }}"
        ))
        .expect_err("只有 trait 声明、没有 impl 时 `spin` 不得解析");
    assert!(matches!(&error, HirError::UnresolvedCall { .. }), "got {error:?}");
}
