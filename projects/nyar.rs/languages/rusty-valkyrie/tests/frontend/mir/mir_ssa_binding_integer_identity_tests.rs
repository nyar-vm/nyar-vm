//! 绑定注解与源值类型的 primitive 身份边界合同。
//!
//! 反例锁定：注解与源值是真正不同的数值类型时，绑定必须留下结构化诊断并被
//! Semantic MIR 合同拒绝，不得改写源值类型规避 `StoreVar` 校验（CAN015）。
//! 正例锁定：`usize` 别名绑定到同一载体身份的源值时自然相等，全程无诊断。

use rusty_valkyrie::{
    MirDiagnostic, MirLowerer, ValkyrieCompiler,
    mir::validate_semantic_module,
};

const PRIMITIVES: &str = r#"
[primitive("std::primitive::u64")] structure u64 { }
[primitive("std::primitive::i64")] structure i64 { }
type usize = u64;
"#;

/// 正例守卫：注解 `usize`（别名 → `u64` 声明身份）绑定返回 `usize` 的合同调用，
/// hint 与 concrete 同形，贯穿 Semantic MIR 与 canonical，且不留任何诊断。
#[test]
fn usize_alias_annotation_binding_reaches_canonical_without_diagnostic() {
    let source = format!(
        r#"{PRIMITIVES}
[host_contract] private micro __length(array: [u64]): usize
micro probe(array: [u64]) -> u64 {{
    let n: usize = __length(array)
    return n
}}
"#
    );
    let hir = ValkyrieCompiler::default().compile_source(&source).expect("正例必须完成 HIR");
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        mir.diagnostics
            .iter()
            .all(|diagnostic| !matches!(diagnostic, MirDiagnostic::BindingAnnotationTypeMismatch { .. })),
        "同一 primitive 身份的别名绑定不得产生数值类型失配诊断: {:?}",
        mir.diagnostics
    );
    validate_semantic_module(&mir).expect("别名绑定必须通过 Semantic MIR 合同");
}

/// 反例：注解 `usize`（→ `u64`）而源值记录为 `i64` 声明身份——归一表示后仍是
/// 真正不同的数值类型，必须结构化拒绝，不得静默放行或改写源值类型。
/// 前端分层防线：HIR 的 `let` 初始化检查先拒；若该检查未来回归，MIR 绑定边界
/// 的 `BindingAnnotationTypeMismatch` 必须接住，两层都不允许绿灯。
#[test]
fn mismatched_integer_binding_annotation_is_rejected_structurally() {
    let source = format!(
        r#"{PRIMITIVES}
[host_contract] private micro __make_i64(): i64
micro probe() -> u64 {{
    let n: usize = __make_i64()
    return 0
}}
"#
    );
    let hir = match ValkyrieCompiler::default().compile_source(&source) {
        Err(rusty_valkyrie::HirError::LetInitializerTypeMismatch { actual, expected, .. }) => {
            assert_eq!(actual, "i64");
            assert_eq!(expected, "u64");
            return;
        }
        Ok(hir) => hir,
        Err(error) => panic!("数值类型失配必须被结构化拒绝，got {error:?}"),
    };
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        mir.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::BindingAnnotationTypeMismatch { name, .. } if name == "n"
        )),
        "HIR 漏放的数值类型失配必须由 BindingAnnotationTypeMismatch 接住: {:?}",
        mir.diagnostics
    );
    validate_semantic_module(&mir).expect_err("结构化诊断必须阻止 Semantic MIR 成功");
}
