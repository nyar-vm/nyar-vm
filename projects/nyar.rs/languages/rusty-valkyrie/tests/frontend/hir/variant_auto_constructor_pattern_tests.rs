//! 块 4：`case` 模式的变体载荷绑定合同。
//!
//! 模式解析走声明身份（`sum_owner` / `sum_variant`），与括号构造、字面量构造共享同一变体身份；
//! 载荷按字段声明顺序绑定，泛型 scrutinee 走既有类型代换链。

use rusty_valkyrie::{
    ValkyrieCompiler,
    types::hir::{HirExprKind, HirExtractorPattern, HirMatchArm, HirModule, HirPattern, ValkyrieType},
    validate_semantic_module,
};

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(source).unwrap_or_else(|error| panic!("源码必须完成 HIR lowering: {error}"))
}

fn compile_to_mir(source: &str) {
    let compiler = ValkyrieCompiler::default();
    let mir = compiler.compile_source_to_mir(source).unwrap_or_else(|error| panic!("源码必须完成 Semantic MIR lowering: {error}"));
    validate_semantic_module(&mir).unwrap_or_else(|error| panic!("Semantic MIR 合同必须通过: {error:?}"));
}

fn match_arms<'a>(module: &'a HirModule, function: &str) -> &'a [HirMatchArm] {
    let function = module.functions.iter().find(|item| item.name.as_str() == function).unwrap_or_else(|| panic!("缺少 {function}"));
    let expr = function.body.expr.as_deref().expect("函数体必须是表达式");
    match &expr.kind {
        HirExprKind::Match { arms, .. } | HirExprKind::Case { arms, .. } => arms,
        other => panic!("函数体必须是 match/case，got {other:?}"),
    }
}

fn variant_pattern<'a>(arm: &'a HirMatchArm, variant: &str) -> &'a HirExtractorPattern {
    let HirPattern::Extractor(HirExtractorPattern::Constructor { name, resolved, .. }) = &arm.pattern
    else {
        panic!("{variant} 分支必须降为变体提取模式，got {:?}", arm.pattern);
    };
    assert_eq!(name.to_string(), variant);
    let resolved = resolved.as_ref().unwrap_or_else(|| panic!("{variant} 分支必须携带已解析调用合同"));
    assert!(resolved.sum_owner.is_some() && resolved.sum_variant.is_some(), "{variant} 分支必须携带 sum_owner/sum_variant");
    match &arm.pattern {
        HirPattern::Extractor(pattern) => pattern,
        _ => unreachable!(),
    }
}

fn pattern_fields(pattern: &HirExtractorPattern) -> &[HirPattern] {
    match pattern {
        HirExtractorPattern::Constructor { fields, .. } => fields,
        other => panic!("必须是 Constructor 提取模式，got {other:?}"),
    }
}

fn pattern_payload(pattern: &HirExtractorPattern) -> &ValkyrieType {
    match pattern {
        HirExtractorPattern::Constructor { resolved, .. } => {
            resolved.as_ref().and_then(|resolved| resolved.extractor_payload_type.as_ref()).expect("模式必须携带载荷类型")
        }
        other => panic!("必须是 Constructor 提取模式，got {other:?}"),
    }
}

const OPTION_UNITE: &str = r#"
unite Option<T> {
    [constructor]
    Some {
        value: T,
    },
    None,
}
"#;

const OPTION_MATCH_FUNCTION: &str = r#"
micro unwrap_or_false(input: Option<bool>) -> bool {
    match input {
        case Some(value):
            value
        case None:
            false
    }
}
"#;

#[test]
fn single_payload_pattern_binds_first_field_and_reaches_semantic_mir() {
    let source = format!("{OPTION_UNITE}{OPTION_MATCH_FUNCTION}");
    let module = compile(&source);
    let arms = match_arms(&module, "unwrap_or_false");
    assert_eq!(arms.len(), 2);

    let some = variant_pattern(&arms[0], "Some");
    assert_eq!(pattern_fields(some).len(), 1, "`case Some(value)` 必须绑定一个字段");
    assert!(matches!(&pattern_fields(some)[0], HirPattern::Variable(name) if name.name.as_str() == "value"));
    assert_eq!(pattern_payload(some), &ValkyrieType::Boolean, "泛型 scrutinee 必须经既有代换推出载荷类型");

    let none = variant_pattern(&arms[1], "None");
    assert!(pattern_fields(none).is_empty());
    assert_eq!(pattern_payload(none), &ValkyrieType::Unit, "零载荷变体的载荷类型必须是 Unit");

    compile_to_mir(&source);
}

const MULTI_FIELD_SOURCE: &str = r#"
unite Pair {
    [constructor]
    Both {
        left: bool,
        right: utf8,
    },
    Neither,
}
micro describe(input: Pair) -> bool {
    match input {
        case Both(left, right):
            left
        case Neither:
            false
    }
}
"#;

#[test]
fn multi_field_pattern_binds_fields_in_declaration_order() {
    let source = MULTI_FIELD_SOURCE;
    let module = compile(source);
    let arms = match_arms(&module, "describe");
    let both = variant_pattern(&arms[0], "Both");
    let fields = pattern_fields(both);
    assert_eq!(fields.len(), 2, "多字段变体必须按声明顺序绑定");
    assert!(matches!(&fields[0], HirPattern::Variable(name) if name.name.as_str() == "left"));
    assert!(matches!(&fields[1], HirPattern::Variable(name) if name.name.as_str() == "right"));
    assert_eq!(
        pattern_payload(both),
        &ValkyrieType::Tuple(vec![ValkyrieType::Boolean, ValkyrieType::Utf8]),
        "多字段载荷必须是按声明顺序排列的 Tuple"
    );
}

#[test]
fn multi_field_pattern_reaches_semantic_mir() {
    compile_to_mir(MULTI_FIELD_SOURCE);
}

const NAMED_FIELD_ORDER_SOURCE: &str = r#"
unite Pair {
    [constructor]
    Both {
        left: bool,
        right: utf8,
    },
    Neither,
}

micro forward(input: Pair) -> bool {
    match input {
        case Both { left, right }:
            left
        case Neither:
            false
    }
}

micro reversed(input: Pair) -> bool {
    match input {
        case Both { right, left }:
            left
        case Neither:
            false
    }
}

micro text_of(input: Pair) -> utf8 {
    match input {
        case Both { right, left }:
            right
        case Neither:
            ""
    }
}
"#;

/// 对象模式按声明字段名绑定，与书写顺序无关。
///
/// `reversed` 与 `text_of` 都以逆序书写对象模式，但槽位仍必须按声明顺序排列：
/// 槽位 0 得到 `left: bool`，槽位 1 得到 `right: utf8`。
/// 若退化为按书写位置绑定，`reversed` 里的 `left` 会被推断成 `utf8`、
/// `text_of` 里的 `right` 会被推断成 `bool`，返回值类型立即失配。
#[test]
fn named_object_pattern_binds_by_declared_field_name_regardless_of_order() {
    let module = compile(NAMED_FIELD_ORDER_SOURCE);
    for function in ["forward", "reversed", "text_of"] {
        let both = variant_pattern(&match_arms(&module, function)[0], "Both");
        let fields = pattern_fields(both);
        assert_eq!(fields.len(), 2, "{function}: 对象模式必须绑定两个字段");
        assert!(
            matches!(&fields[0], HirPattern::Variable(name) if name.name.as_str() == "left"),
            "{function}: 槽位 0 必须绑定声明字段 `left`，实际 {:?}",
            fields[0]
        );
        assert!(
            matches!(&fields[1], HirPattern::Variable(name) if name.name.as_str() == "right"),
            "{function}: 槽位 1 必须绑定声明字段 `right`，实际 {:?}",
            fields[1]
        );
        assert_eq!(
            pattern_payload(both),
            &ValkyrieType::Tuple(vec![ValkyrieType::Boolean, ValkyrieType::Utf8]),
            "{function}: 槽位类型必须按声明顺序 left=bool, right=utf8"
        );
    }
    // 类型级闭环：`reversed` 从 `left` 返回 bool，`text_of` 从 `right` 返回 utf8。
    // 若按书写位置绑定，两个函数的返回类型都会失配，MIR lowering 必须失败。
    compile_to_mir(NAMED_FIELD_ORDER_SOURCE);
}

#[test]
fn zero_payload_pattern_matches_by_tag_and_reaches_semantic_mir() {
    let source = r#"
unite Flag {
    Off,
    On,
}
micro is_off(input: Flag) -> bool {
    match input {
        case Off:
            true
        case On:
            false
    }
}
"#;
    let module = compile(source);
    let arms = match_arms(&module, "is_off");
    let off = variant_pattern(&arms[0], "Off");
    assert!(pattern_fields(off).is_empty());
    assert_eq!(pattern_payload(off), &ValkyrieType::Unit);

    compile_to_mir(source);
}

#[test]
fn bracket_call_and_literal_construction_match_identically() {
    let source = format!(
        r#"{OPTION_UNITE}
micro bracket_value() -> bool {{
    let value = Some(true)
    match value {{
        case Some(inner):
            inner
        case None:
            false
    }}
}}
micro literal_value() -> bool {{
    let value = Some {{ value: true }}
    match value {{
        case Some(inner):
            inner
        case None:
            false
    }}
}}
"#
    );
    let module = compile(&source);

    let bracket_some = variant_pattern(&match_arms(&module, "bracket_value")[0], "Some");
    let literal_some = variant_pattern(&match_arms(&module, "literal_value")[0], "Some");
    let (bracket_resolved, literal_resolved) = match (bracket_some, literal_some) {
        (
            HirExtractorPattern::Constructor { resolved: Some(bracket), .. },
            HirExtractorPattern::Constructor { resolved: Some(literal), .. },
        ) => (bracket, literal),
        _ => panic!("两种写法的 Some 分支都必须解析"),
    };
    assert_eq!(bracket_resolved.sum_owner, literal_resolved.sum_owner, "两种写法必须归属同一 unite 声明");
    assert_eq!(bracket_resolved.sum_variant, literal_resolved.sum_variant, "两种写法必须是同一变体身份");
    assert_eq!(pattern_payload(bracket_some), pattern_payload(literal_some), "两种写法的载荷类型必须一致");

    compile_to_mir(&source);
}
