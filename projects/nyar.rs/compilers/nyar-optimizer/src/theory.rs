//! `core.*` 语义操作的默认重写理论与常量折叠解释。
//!
//! 操作命名空间属于语言侧合同。本模块把 `core` 算术恒等式与折叠语义
//! 组装成中立底座消费的 [`TermRewrite`] 与 [`ConstantInterpreter`]，
//! 底座本身不内置任何语言的操作名。

use crate::RewriteTheory;
use compiler_optimize::{ConstantInterpreter, Identifier, QualifiedName, RewritePhase, TermRewrite, parse_pattern};

/// `core.*` 算术常量折叠解释。
///
/// 折叠语义与 `core` 操作的既定饱和行为保持一致，包括 `div` / `mod`
/// 以右侧操作数为被除数或被除数的历史口径。
pub struct CoreConstantInterpreter;

impl ConstantInterpreter for CoreConstantInterpreter {
    fn fold_binary(&self, operator: &QualifiedName, left: i64, right: i64) -> Option<i64> {
        match (operator.parts().first()?.as_str(), operator.parts().get(1)?.as_str()) {
            ("core", "add") => Some(left.saturating_add(right)),
            ("core", "sub") => Some(left.saturating_sub(right)),
            ("core", "mul") => Some(left.saturating_mul(right)),
            ("core", "div") => right.checked_div(left),
            ("core", "mod") => right.checked_rem(left),
            _ => None,
        }
    }

    fn fold_unary(&self, operator: &QualifiedName, operand: i64) -> Option<i64> {
        if operator.parts().len() == 2 && operator.parts()[0].as_str() == "core" && operator.parts()[1].as_str() == "neg" {
            Some(-operand)
        }
        else {
            None
        }
    }
}

/// `core.lit_N` 字面量 flat 名称。
pub fn core_lit_name(value: i64) -> QualifiedName {
    QualifiedName::new(vec![Identifier::new("core"), Identifier::new(&format!("lit_{value}"))])
}

/// 内置 `core` 代数重写理论。
pub fn builtin_core_rewrite_theory() -> RewriteTheory {
    let mut theory = RewriteTheory::default();
    theory.term_rewrites.extend(builtin_core_term_rewrites());
    theory
}

/// 内置 `core` 结构化项级重写。
pub fn builtin_core_term_rewrites() -> Vec<TermRewrite> {
    vec![
        TermRewrite::new(
            "core.add-identity-left",
            parse_pattern("core.add(0, ?x)").expect("valid core.add identity pattern"),
            parse_pattern("?x").expect("valid variable pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.add-identity-right",
            parse_pattern("core.add(?x, 0)").expect("valid core.add identity pattern"),
            parse_pattern("?x").expect("valid variable pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.mul-identity-left",
            parse_pattern("core.mul(1, ?x)").expect("valid core.mul identity pattern"),
            parse_pattern("?x").expect("valid variable pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.mul-identity-right",
            parse_pattern("core.mul(?x, 1)").expect("valid core.mul identity pattern"),
            parse_pattern("?x").expect("valid variable pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.mul-zero-left",
            parse_pattern("core.mul(0, ?x)").expect("valid core.mul zero pattern"),
            parse_pattern("0").expect("valid literal pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.mul-zero-right",
            parse_pattern("core.mul(?x, 0)").expect("valid core.mul zero pattern"),
            parse_pattern("0").expect("valid literal pattern"),
            RewritePhase::Saturate,
        ),
        TermRewrite::new(
            "core.sub-self",
            parse_pattern("core.sub(?x, ?x)").expect("valid core.sub self pattern"),
            parse_pattern("0").expect("valid literal pattern"),
            RewritePhase::Saturate,
        ),
    ]
}
