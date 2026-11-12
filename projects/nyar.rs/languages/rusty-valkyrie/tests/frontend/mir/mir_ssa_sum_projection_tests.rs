//! sum 构造 lowering 只投影 HIR 已结算的完整事实：自定义 sum、不同 binder 排列、
//! 缺事实的结构化门（带 source/span），以及字节字面量由 HIR settle 唯一权威定型。
//!
//! 全部夹具使用自定义名（`Box2`/`Pair`/`Outcome`/`Full`/`First`/`Bad`），不触碰任何
//! 标准库名称，证明路径不存在 `Result`/`Option`/`Some`/`Fine`/`Fail` 特判。

use rusty_valkyrie::{
    MirLowerer, ValkyrieCompiler,
    mir::ssa::{MirDiagnostic, MirOperation},
    types::hir::{HirExprKind, ValkyrieType},
};

use crate::primitives::SourceFixtureCompile;

/// 降低源码并取出目标函数里第一条 `SumNew` 的「实例化实参 + payload 类型」。
fn first_sum_new(hir: &rusty_valkyrie::types::hir::HirModule, suffix: &str) -> (Vec<ValkyrieType>, Option<ValkyrieType>) {
    let mir = MirLowerer::lower_module_semantic(hir);
    assert!(mir.diagnostics.is_empty(), "MIR 必须零诊断：{:?}", mir.diagnostics);
    let function = mir.functions.iter().find(|item| item.display_symbol.ends_with(suffix)).unwrap_or_else(|| panic!("缺少函数 {suffix}"));
    for block in &function.blocks {
        for instruction in &block.instructions {
            if let MirOperation::SumNew { type_args, payload_type, .. } = &instruction.kind {
                return (type_args.clone(), payload_type.clone());
            }
        }
    }
    panic!("函数 {suffix} 必须产出 SumNew");
}

/// 自定义单 binder sum：实例化实参与 payload 都来自 HIR 结算，MIR 零诊断。
#[test]
fn custom_sum_constructor_projects_settled_instantiation() {
    let hir = ValkyrieCompiler::default()
        .compile_fixture_source(
            "unite Box2<T> {\n    [constructor]\n    Full { v: T },\n}\n\nmicro make(): Box2<u64> {\n    return Full { v: 7 }\n}",
        )
        .expect("自定义 sum 必须完成 HIR");
    let (type_args, payload_type) = first_sum_new(&hir, "::make");
    assert_eq!(type_args.len(), 1, "实参表必须逐槽来自已结算实例化: {type_args:?}");
    assert_eq!(payload_type.as_ref(), Some(&type_args[0]), "payload 类型必须等于已结算形参类型");
}

/// 不同 binder 排列：payload 落在哪个槽由声明与已结算实参决定，不由 payload 类型回填。
#[test]
fn binder_permutation_keeps_slots_from_declaration() {
    let source = "unite Pair<A, B> {\n    [constructor]\n    First { a: A },\n    [constructor]\n    Second { b: B },\n}\n\nmicro one(): Pair<u64, bool> {\n    return First { a: 1 }\n}\n\nmicro two(): Pair<u64, bool> {\n    return Second { b: true }\n}";
    let hir = ValkyrieCompiler::default().compile_fixture_source(source).expect("双 binder 自定义 sum 必须完成 HIR");
    let (first_args, first_payload) = first_sum_new(&hir, "::one");
    let (second_args, second_payload) = first_sum_new(&hir, "::two");
    assert_eq!(first_args, second_args, "同一实例化在两个变体上必须给出同一实参表: {first_args:?} vs {second_args:?}");
    assert_eq!(first_args.len(), 2);
    assert_eq!(first_payload.as_ref(), Some(&first_args[0]), "`First` 的 payload 必须是槽 A 的已结算类型");
    assert_eq!(second_payload.as_ref(), Some(&second_args[1]), "`Second` 的 payload 必须是槽 B 的已结算类型");
}

/// 双 binder 且 payload 在第二 binder：实参表仍按声明顺序整体投影，不逐槽回填。
#[test]
fn payload_in_second_binder_does_not_rewrite_argument_table() {
    let source =
        "unite Outcome<T, E> {\n    [constructor]\n    Bad { err: E },\n}\n\nmicro probe(): Outcome<bool, u64> {\n    return Bad { err: 9 }\n}";
    let hir = ValkyrieCompiler::default().compile_fixture_source(source).expect("payload 在第二 binder 的自定义 sum 必须完成 HIR");
    let (type_args, payload_type) = first_sum_new(&hir, "::probe");
    assert_eq!(type_args.len(), 2, "两个 binder 的实参必须都来自已结算实例化: {type_args:?}");
    assert_eq!(payload_type.as_ref(), Some(&type_args[1]));
}

/// 缺事实门：把已解析构造与函数返回类型上的实例化实参剥掉（模拟 HIR 未结算完整事实），
/// MIR 必须给出带真实 source/span 的结构化诊断，禁止从 binder 表合成实参继续。
#[test]
fn constructor_without_settled_type_args_fails_closed_with_span() {
    let mut hir = ValkyrieCompiler::default()
        .compile_fixture_source("unite Box2<T> {\n    [constructor]\n    Full { v: T },\n}\n\nmicro make(): Box2<u64> {\n    Full { v: 7 }\n}")
        .expect("夹具必须先完成 HIR");
    let function = hir.functions.iter_mut().find(|item| item.name.as_str() == "make").expect("缺少 make 函数");
    strip_nominal_arguments(&mut function.return_type, "函数返回类型");
    let tail = function.body.expr.as_mut().expect("尾表达式必须存在");
    let HirExprKind::Construct { resolved: Some(resolved), .. } = &mut tail.kind
    else {
        panic!("构造必须已被 HIR 解析: {:?}", tail.kind);
    };
    strip_nominal_arguments(&mut resolved.return_type, "已解析构造返回类型");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let diagnostic = mir
        .diagnostics
        .iter()
        .find(|item| matches!(item, MirDiagnostic::UnresolvedSumInstanceFacts { .. }))
        .unwrap_or_else(|| panic!("剥掉已结算实参后必须触发带 span 的结构化诊断：{:?}", mir.diagnostics));
    match diagnostic {
        MirDiagnostic::UnresolvedSumInstanceFacts { span, context } => {
            assert!(span.span.end > span.span.start, "诊断必须携带真实源码区间: {span:?}");
            assert!(context.contains("binder"), "诊断必须指明缺额事实: {context}");
        }
        other => panic!("预期 UnresolvedSumInstanceFacts，实际 {other:?}"),
    }
}

/// 把带实参的 `Nominal` 剥成无实参裸名义（夹具模拟「HIR 未结算实例化」专用）。
fn strip_nominal_arguments(ty: &mut ValkyrieType, label: &str) {
    let ValkyrieType::Nominal { declaration, arguments, primitive_identity } = std::mem::replace(ty, ValkyrieType::AutoType)
    else {
        panic!("{label}必须是带实参的 Nominal");
    };
    assert!(!arguments.is_empty(), "夹具前提：{label} 已结算实例化实参");
    *ty = ValkyrieType::Nominal { declaration, arguments: Vec::new(), primitive_identity };
}

/// 字节字面量类型由 HIR `settle_byte` 唯一结算：整数目标按范围采纳，装不下时给出
/// 带 span 的结构化诊断；MIR 不再按上下文形状或范围另选字节类型。
#[test]
fn byte_literal_type_settles_in_hir_authority() {
    let hir = ValkyrieCompiler::default()
        .compile_fixture_source("micro probe(): i32 {\n    let wide: i32 = b'0'\n    return wide\n}")
        .expect("字节字面量绑定整数形参必须完成 HIR");
    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(mir.diagnostics.is_empty(), "i32 目标在字节值域内必须零诊断：{:?}", mir.diagnostics);
    let source = "micro probe(): i8 {\n    let bad: i8 = b'\\xFF'\n    return bad\n}";
    let hir = ValkyrieCompiler::default().compile_fixture_source(source).expect("越界字节字面量必须先通过 HIR");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let message = mir
        .diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}"))
        .find(|text| text.contains("out of range"))
        .expect("i8 装不下的字节字面量必须给出范围结构化诊断");
    assert!(message.contains("byte literal"), "诊断必须指明字节字面量: {message}");
}
