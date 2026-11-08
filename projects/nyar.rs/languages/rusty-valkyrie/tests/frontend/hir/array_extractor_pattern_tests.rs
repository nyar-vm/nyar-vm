//! 数组 extractor 模式（`case [head, ...tail]`）的 HIR lowering 合同。
//!
//! oak `Pattern::Array` 降为 `HirPattern::Extractor(HirExtractorPattern::Array)`：
//! prefix/rest/suffix 按位落槽，被调路径遵循 `[T]` 名义 owner 的 `Array.extractor`
//! 约定，调用解析前 `resolved` 为空。完整管线下 scrutinee 无已声明 extractor 时，
//! 由 extractor 收集器产出结构化失败（fail-closed，不多报不错报）。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::{
        diagnostics::pending,
        hir::lowering::DEFAULT_COMPILE_ARCH,
    },
    types::hir::{HirExprKind, HirExtractorPattern, HirMatchArm, HirModule, HirPattern},
};

fn lower_without_call_resolution(source: &str) -> HirModule {
    ValkyrieCompiler::default()
        .parse_source_group_without_call_resolution(&crate::primitives::with_primitives(source), &[], None, DEFAULT_COMPILE_ARCH)
        .unwrap_or_else(|error| panic!("源码必须完成 HIR 物化: {error}"))
}

fn match_arms<'a>(module: &'a HirModule, function: &str) -> &'a [HirMatchArm] {
    let function = module.functions.iter().find(|item| item.name.as_str() == function).unwrap_or_else(|| panic!("缺少 {function}"));
    let expr = function.body.expr.as_deref().expect("函数体必须是表达式");
    match &expr.kind {
        HirExprKind::Match { arms, .. } | HirExprKind::Case { arms, .. } => arms,
        other => panic!("函数体必须是 match/case，got {other:?}"),
    }
}

fn variable_name(pattern: &HirPattern) -> &str {
    match pattern {
        HirPattern::Variable(identifier) => identifier.name.as_str(),
        other => panic!("必须是变量绑定模式，got {other:?}"),
    }
}

/// 正例：`case [head, ...tail]` 降为 prefix + rest 的数组 extractor。
#[test]
fn array_pattern_with_rest_lowers_to_array_extractor() {
    let source = r#"
micro first_or_zero(data: [i64]) -> i64 {
    match data {
        case [head, ...tail]:
            head
        case _:
            0
    }
}
"#;
    let module = lower_without_call_resolution(source);
    let arms = match_arms(&module, "first_or_zero");
    let HirPattern::Extractor(HirExtractorPattern::Array { canonical_callee, prefix, rest, suffix, resolved }) = &arms[0].pattern
    else {
        panic!("首分支必须降为数组 extractor 模式，got {:?}", arms[0].pattern);
    };
    let callee_parts: Vec<&str> = canonical_callee.parts().iter().map(|part| part.as_str()).collect();
    assert_eq!(callee_parts, ["Array", "extractor"], "被调路径遵循 `[T]` 名义 owner 的 extractor 约定");
    assert!(resolved.is_none(), "调用解析前不得携带 resolved");
    assert_eq!(prefix.len(), 1, "prefix: {prefix:?}");
    assert_eq!(variable_name(&prefix[0]), "head");
    let rest = rest.as_ref().expect("必须携带 rest 绑定");
    assert_eq!(rest.name.as_str(), "tail");
    assert!(suffix.is_empty(), "suffix: {suffix:?}");
}

/// 正例：`case [a, b, ...rest, c]` 完整 prefix/rest/suffix 形态。
#[test]
fn array_pattern_prefix_rest_suffix_lowers_in_order() {
    let source = r#"
micro edge_sum(data: [i64]) -> i64 {
    match data {
        case [a, b, ...rest, c]:
            a + b + c
        case _:
            0
    }
}
"#;
    let module = lower_without_call_resolution(source);
    let arms = match_arms(&module, "edge_sum");
    let HirPattern::Extractor(HirExtractorPattern::Array { prefix, rest, suffix, .. }) = &arms[0].pattern
    else {
        panic!("首分支必须降为数组 extractor 模式，got {:?}", arms[0].pattern);
    };
    let prefix_names: Vec<&str> = prefix.iter().map(variable_name).collect();
    assert_eq!(prefix_names, ["a", "b"]);
    let rest = rest.as_ref().expect("必须携带 rest 绑定");
    assert_eq!(rest.name.as_str(), "rest");
    let suffix_names: Vec<&str> = suffix.iter().map(variable_name).collect();
    assert_eq!(suffix_names, ["c"]);
}

/// 反例：scrutinee 无已声明 `Array.extractor` 时，完整管线结构化失败并记入诊断汇。
#[test]
fn array_pattern_without_declared_extractor_fails_closed() {
    pending::clear_pending_diagnostics();
    let source = r#"
micro first_or_zero(data: [i64]) -> i64 {
    match data {
        case [head, ...tail]:
            head
        case _:
            0
    }
}
"#;
    let error = ValkyrieCompiler::default()
        .compile_source(&crate::primitives::with_primitives(source))
        .expect_err("未声明 `Array.extractor` 的数组模式必须结构化失败");
    assert!(error.to_string().contains("Array.extractor"), "失败必须指名缺失的 extractor: {error}");
    let collected = pending::take_pending_diagnostics();
    assert!(
        collected.diagnostics.iter().any(|diagnostic| diagnostic.message.contains("Array.extractor")),
        "extractor 收集器必须把失败记入诊断汇: {collected:?}"
    );
}
