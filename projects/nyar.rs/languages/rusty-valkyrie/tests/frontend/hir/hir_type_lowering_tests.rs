use rusty_valkyrie::{
    CompilerSourceGroup, HirError, SourceID, ValkyrieCompiler,
    frontend::ast::{Identifier, NamePath, TypeExpression, type_nodes::TypeUnaryNode},
    hir::type_lowering::{canonical_builtin_type, lower_type_expression, validate_type_expression},
    hir::lowering::errors::{ArrayExtentFailure, TupleLabelingFailure},
    hir::lowering::DEFAULT_COMPILE_ARCH,
    types::{
        Identifier as HirIdentifier,
        hir::{HirModule, HirStruct, ValkyrieType},
    },
};

use oak_core::Range;
use oak_valkyrie::lexer::ValkyrieTokenType;
use rusty_valkyrie::compiler::syntax::oak::ast::TermExpression as TypeExtent;

fn oak_span(len: usize) -> Range<usize> {
    Range { start: 0, end: len }
}

fn path_type(name: &str) -> TypeExpression {
    TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![Identifier { name: name.to_string(), span: oak_span(name.len()) }],
        span: oak_span(name.len()),
    }))
}

#[test]
fn named_tuple_type_lowers_to_labeled_tuple() {
    use rusty_valkyrie::compiler::syntax::oak::ast::type_nodes::{TupleType, TupleTypeElement};

    let tuple = TypeExpression::Tuple(Box::new(TupleType {
        elements: vec![
            TupleTypeElement {
                name: Some(Identifier { name: "ordinal".into(), span: oak_span(7) }),
                ty: path_type("usize"),
                span: oak_span(14),
            },
            TupleTypeElement { name: Some(Identifier { name: "value".into(), span: oak_span(5) }), ty: path_type("u32"), span: oak_span(7) },
        ],
        span: oak_span(30),
    }));
    assert_eq!(
        lower_type_expression(&tuple).expect("named tuple lowers"),
        ValkyrieType::LabeledTuple(vec![
            rusty_valkyrie::types::hir::HirTupleField {
                name: HirIdentifier::new("ordinal"),
                ty: ValkyrieType::Named(HirIdentifier::new("usize")),
            },
            rusty_valkyrie::types::hir::HirTupleField {
                name: HirIdentifier::new("value"),
                ty: ValkyrieType::Named(HirIdentifier::new("u32")),
            },
        ])
    );
}

fn tuple_type(elements: Vec<(Option<&str>, &str)>) -> TypeExpression {
    use rusty_valkyrie::compiler::syntax::oak::ast::type_nodes::{TupleType, TupleTypeElement};

    TypeExpression::Tuple(Box::new(TupleType {
        elements: elements
            .into_iter()
            .map(|(name, ty)| TupleTypeElement {
                name: name.map(|name| Identifier { name: name.to_string(), span: oak_span(name.len()) }),
                ty: path_type(ty),
                span: oak_span(0),
            })
            .collect(),
        span: oak_span(0),
    }))
}

/// 反例：部分命名的元组必须在类型构造处结构化失败。
///
/// 允许它会让 `LabeledTuple` 里出现无名占位，正是设计里明确否掉的非法状态。
#[test]
fn mixed_labeled_tuple_is_structurally_rejected() {
    for tuple in [vec![(Some("a"), "i32"), (None, "i64")], vec![(None, "i32"), (Some("b"), "i64")]] {
        let error = lower_type_expression(&tuple_type(tuple)).expect_err("混合命名元组必须拒绝");
        assert!(matches!(error, HirError::InvalidTupleLabeling { failure: TupleLabelingFailure::Mixed, .. }), "error={error:?}");
    }
}

/// 反例：全命名但标签重复，类型身份无法唯一确定位置。
#[test]
fn duplicate_tuple_labels_are_structurally_rejected() {
    let error =
        lower_type_expression(&tuple_type(vec![(Some("a"), "i32"), (Some("a"), "i64")])).expect_err("重复标签必须拒绝");
    assert!(
        matches!(error, HirError::InvalidTupleLabeling { failure: TupleLabelingFailure::DuplicateLabel { ref name }, .. } if name == "a"),
        "error={error:?}"
    );
}

/// 正例：全无名元组仍是独立的无标签形态，一个字节都不改。
#[test]
fn unlabeled_tuple_stays_positional_tuple() {
    assert_eq!(
        lower_type_expression(&tuple_type(vec![(None, "i32"), (None, "i32")])).expect("无标签元组合法"),
        ValkyrieType::Tuple(vec![ValkyrieType::Named(HirIdentifier::new("i32")), ValkyrieType::Named(HirIdentifier::new("i32"))])
    );
    assert_eq!(lower_type_expression(&tuple_type(Vec::new())).expect("空元组合法"), ValkyrieType::Unit);
}

#[test]
fn rejects_ambiguous_string_type_names() {
    for name in ["string", "str", "String"] {
        let error = validate_type_expression(&path_type(name)).expect_err("ambiguous string types must be rejected");
        let message = error.to_string();
        assert!(message.contains("ambiguous text type") || message.contains("forbidden"), "message={message}");
        assert!(message.contains("utf8") && message.contains("utf16") && message.contains("c_str"), "message={message}");
    }
}

#[test]
fn accepts_explicit_text_encodings() {
    for name in ["utf8", "utf16", "c_str"] {
        validate_type_expression(&path_type(name)).expect("explicit text encodings must be accepted");
    }
    assert_eq!(canonical_builtin_type("utf8"), Some(ValkyrieType::Utf8));
    assert_eq!(canonical_builtin_type("utf16"), Some(ValkyrieType::Utf16));
    assert_eq!(canonical_builtin_type("c_str"), Some(ValkyrieType::Named(HirIdentifier::new("c_str"))));
    assert!(canonical_builtin_type("string").is_none());
}

#[test]
fn std_text_utf16_path_lowers_to_utf16_builtin() {
    let path = TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![
            Identifier { name: "std".into(), span: oak_span(3) },
            Identifier { name: "text".into(), span: oak_span(4) },
            Identifier { name: "utf16".into(), span: oak_span(5) },
        ],
        span: oak_span(14),
    }));
    assert_eq!(lower_type_expression(&path).expect("std text path lowers"), ValkyrieType::Utf16);
}

#[test]
fn unknown_generic_apply_base_is_rejected_before_nominal_binding() {
    use rusty_valkyrie::compiler::syntax::oak::ast::type_nodes::ApplyType;

    // `Option` 不在作用域内：不得退化成 `Named("Option")` 带进 canonical，必须结构化失败。
    let apply = TypeExpression::Apply(Box::new(ApplyType { base: path_type("Option"), arguments: vec![path_type("i32")], span: oak_span(0) }));
    let error = lower_type_expression(&apply).expect_err("未知泛型基底必须结构化失败");
    assert!(matches!(error, HirError::TypeNotFound { .. }), "error={error:?}");
}

#[test]
fn named_array_apply_is_not_structural_bracket_sugar() {
    use rusty_valkyrie::compiler::syntax::oak::ast::type_nodes::ApplyType;
    let named = TypeExpression::Apply(Box::new(ApplyType { base: path_type("usize"), arguments: vec![path_type("i32")], span: oak_span(0) }));
    assert_eq!(
        lower_type_expression(&named).expect("named apply lowers"),
        ValkyrieType::Apply(Box::new(ValkyrieType::Named(HirIdentifier::new("usize"))), vec![ValkyrieType::Named(HirIdentifier::new("i32"))],)
    );
}

#[test]
fn dotted_module_type_path_is_rejected_before_canonical() {
    let path = TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![
            Identifier { name: "leetcode".into(), span: oak_span(8) },
            Identifier { name: "two_sum".into(), span: oak_span(7) },
            Identifier { name: "Solution".into(), span: oak_span(8) },
        ],
        span: oak_span(25),
    }));
    // 多段限定名不得以 `QualifiedPath` 旁路进入 canonical：短名无法在作用域内绑定即结构化失败。
    let error = lower_type_expression(&path).expect_err("未绑定的限定类型名必须结构化失败");
    assert!(matches!(error, HirError::TypeNotFound { .. }), "error={error:?}");
}

#[test]
fn core_prefixed_type_paths_are_rejected_after_core_merged_into_std() {
    // 两段：`core` 不再是模块根前缀，基底名在作用域内无绑定。
    let two_part = TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![Identifier { name: "core".into(), span: oak_span(4) }, Identifier { name: "Solution".into(), span: oak_span(8) }],
        span: oak_span(13),
    }));
    let error = lower_type_expression(&two_part).expect_err("`core::` 已并入 `std`，必须结构化失败");
    assert!(matches!(error, HirError::TypeNotFound { .. }), "error={error:?}");

    // 三段：`core::math::Random` 末段同样无法绑定，不得静默回退。
    let three_part = TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![
            Identifier { name: "core".into(), span: oak_span(4) },
            Identifier { name: "math".into(), span: oak_span(4) },
            Identifier { name: "Random".into(), span: oak_span(6) },
        ],
        span: oak_span(15),
    }));
    let error = lower_type_expression(&three_part).expect_err("`core::math::Random` 必须结构化失败");
    assert!(matches!(error, HirError::TypeNotFound { .. }), "error={error:?}");
}

#[test]
fn std_prefixed_builtin_path_still_lowers() {
    let path = TypeExpression::Namepath(Box::new(NamePath {
        parts: vec![
            Identifier { name: "std".into(), span: oak_span(3) },
            Identifier { name: "primitive".into(), span: oak_span(9) },
            Identifier { name: "usize".into(), span: oak_span(5) },
        ],
        span: oak_span(18),
    }));
    assert!(lower_type_expression(&path).is_ok(), "`std::primitive::usize` 仍须按 builtin 短名规则解析");
}

#[test]
fn bracket_sugar_lowers_to_array() {
    let sugar = TypeExpression::Unary(Box::new(TypeUnaryNode {
        operator: ValkyrieTokenType::BracketL,
        base: path_type("i32"),
        length: None,
        span: oak_span(0),
    }));
    let expected = ValkyrieType::Array(Box::new(ValkyrieType::Named(HirIdentifier::new("i32"))));
    assert_eq!(lower_type_expression(&sugar).expect("bracket sugar lowers"), expected);
}

// `byte`/`sbyte` 不在 std 声明里，也不是规范拼写，因此不在数值族夹具中。
const NUMERIC_PRIMITIVE_NAMES: [&str; 15] =
    ["i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "isize", "usize", "f32", "f64", "f128"];

fn compile(source: &str) -> Result<HirModule, HirError> {
    ValkyrieCompiler::new(SourceID::default()).compile_source(source)
}

fn find_struct<'a>(hir: &'a HirModule, name: &str) -> &'a HirStruct {
    hir.structs.iter().find(|item| item.name.as_str() == name).unwrap_or_else(|| panic!("缺少结构 `{name}`"))
}

fn declaration_of(hir: &HirModule, name: &str) -> nyar_types::ItemId {
    find_struct(hir, name).constructor_declaration.unwrap_or_else(|| panic!("结构 `{name}` 须已分配 constructor_declaration"))
}

/// 正例：数值名 lowering 为声明引用，不生产 carrier。
#[test]
fn numeric_primitive_names_lower_to_declaration_reference_not_carrier() {
    for name in NUMERIC_PRIMITIVE_NAMES {
        assert!(canonical_builtin_type(name).is_none(), "`{name}` 不是无声明身份的内建");
        assert_eq!(
            lower_type_expression(&path_type(name)).expect("数值名必须是待绑定的声明引用"),
            ValkyrieType::Named(HirIdentifier::new(name)),
            "`{name}` 必须是待绑定的声明引用，不得按名字生产低层 carrier"
        );
    }
}

/// 反例：规范外拼写不是数值声明族，也没有编码内建身份，必须结构化失败。
///
/// 这些名字此前被名字表当作 primitive 放行；删除别名表后它们只能走
/// `resolve_named_type` 的声明解析，未声明即 `TypeNotFound`。
#[test]
fn spec_external_primitive_spellings_fail_closed() {
    for name in [
        "byte", "sbyte", "int", "Int", "long", "Long", "ulong", "uint", "UInt", "Double", "double", "Bool", "boolean", "float32", "float",
        "Float",
    ] {
        assert!(canonical_builtin_type(name).is_none(), "`{name}` 不得被当成无声明身份内建");
        assert!(lower_type_expression(&path_type(name)).is_err(), "未声明的 `{name}` 必须结构化失败，不得被名字表放行");
    }
}

/// 正例：可见的 `std/primitive` 声明把 `Named` 折叠成该内建标量的唯一身份，
/// 声明本身只是宿主绑定，不得在类型位置产生第二个 `Nominal`。
#[test]
fn declared_numeric_primitive_folds_to_builtin_identity() {
    let hir = compile(
        r#"
[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u64")]
structure u64 { }

micro add(lhs: i32, rhs: i32) -> i32 {
    return lhs
}

micro index(n: u64) -> u64 {
    return n
}
"#,
    )
    .expect("已声明的数值 primitive 必须绑定为内建身份");

    let add = hir.functions.iter().find(|function| function.name.as_str() == "add").expect("缺少 `add`");
    for param in &add.params {
        assert_eq!(param.ty, ValkyrieType::Integer32 { signed: true }, "形参类型必须是内建 `i32` 身份，不得是 `Nominal`");
    }
    assert_eq!(add.return_type, ValkyrieType::Integer32 { signed: true });
    assert!(!matches!(add.return_type, ValkyrieType::Nominal { .. }), "`i32` 注解不得产生 `Nominal`");

    let index = hir.functions.iter().find(|function| function.name.as_str() == "index").expect("缺少 `index`");
    // `u64` 的 `[primitive]` 声明同样只做宿主绑定，注解直接落到无符号 64 位载体。
    assert_eq!(index.params[0].ty, ValkyrieType::Integer64 { signed: false });
    assert_eq!(index.return_type, ValkyrieType::Integer64 { signed: false });
}

/// 反例：没有可见 `[primitive]` 声明时，引用处的 primitive 拼写必须在绑定期
/// 结构化失败，而不是残留 `Named("i32")` 让下游身份表按短名接住。
#[test]
fn undeclared_numeric_primitive_fails_without_carrier() {
    let error = compile(
        r#"
micro probe(x: i32) -> i32 {
    return x
}
"#,
    )
    .expect_err("未声明 primitive 不得放行");

    assert!(matches!(error, HirError::TypeNotFound { ref name, .. } if name == "i32"), "error={error:?}");
}

/// 真实形态证明：primitive 声明由依赖组（std 包）提供，入口组自身零 `[primitive]` 声明，
/// 仍能按 carrier 身份编译。声明随依赖组语义导出进入消费组，见
/// `hir/lowering/mod.rs` 的 `HirDependencySemanticExport` 装配；入口组不需要逐文件自备。
#[test]
fn dependency_group_primitive_declaration_reaches_entry_without_local_spelling() {
    let groups = [
        CompilerSourceGroup::single_source("std", "std", crate::primitives::PRIMITIVES),
        CompilerSourceGroup::single_source("app", "app", "[export] micro probe(x: i32) -> i32 { return x }")
            .as_entry()
            .with_direct_dependencies(vec!["std".into()]),
    ];
    let modules = ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .expect("依赖组声明的 primitive 必须可供入口组使用");
    let probe = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .find(|function| function.name.as_str() == "probe")
        .expect("入口组函数必须完成 HIR");
    assert_eq!(probe.params[0].ty, ValkyrieType::Integer32 { signed: true });
    assert_eq!(probe.return_type, ValkyrieType::Integer32 { signed: true });
}

fn int_literal(value: &str) -> TypeExtent {
    TypeExtent::IntegerLiteral { value: value.to_string(), span: oak_span(value.len()) }
}

fn tuple_extent(items: Vec<TypeExtent>) -> TypeExtent {
    TypeExtent::TupleLiteral { items, span: oak_span(0) }
}

fn bracket_type(element: &str, length: Option<TypeExtent>) -> TypeExpression {
    TypeExpression::Unary(Box::new(TypeUnaryNode {
        operator: ValkyrieTokenType::BracketL,
        base: path_type(element),
        length,
        span: oak_span(0),
    }))
}

/// 正例：裸整数 extent 保持栈类型 `FixedArray`，与既有 `[T; N]` 完全一致。
#[test]
fn fixed_array_bare_integer_extent_stays_stack_type() {
    assert_eq!(
        lower_type_expression(&bracket_type("i32", Some(int_literal("4")))).expect("裸整数 extent 合法"),
        ValkyrieType::FixedArray { element: Box::new(ValkyrieType::Named(HirIdentifier::new("i32"))), length: 4 }
    );
}

/// 正例：`[T]` 仍折叠成结构化 `Array`，不因新增形状语法改道。
#[test]
fn unsized_array_extent_stays_structural_array() {
    assert_eq!(
        lower_type_expression(&bracket_type("i32", None)).expect("无 extent 合法"),
        ValkyrieType::Array(Box::new(ValkyrieType::Named(HirIdentifier::new("i32"))))
    );
}

/// 正例：整数元组 extent 落到唯一产生点：按维数折成 `Array1D/2D/3D<T>` 应用。
///
/// `[T; (W, H)]` 与 `Array2D<T, (W, H)>` 都解析到同一个 `Array2D` 声明身份，
/// 不另起一套 `ValkyrieType` 形态。
#[test]
fn tuple_extent_lowers_to_dimension_wrapper_application() {
    for (extent, name) in [
        (vec![3u32], "Array1D"),
        (vec![2, 3], "Array2D"),
        (vec![2, 3, 4], "Array3D"),
    ] {
        let items = extent.into_iter().map(|dimension| int_literal(&dimension.to_string())).collect();
        let lowered = lower_type_expression(&bracket_type("i32", Some(tuple_extent(items)))).expect("整数元组 extent 合法");
        assert_eq!(
            lowered,
            ValkyrieType::Apply(
                Box::new(ValkyrieType::Named(HirIdentifier::new(name))),
                vec![ValkyrieType::Named(HirIdentifier::new("i32"))],
            ),
            "维数应折成 `{name}`"
        );
    }
}

/// 反例：非法 extent 一律结构化拒绝，不再静默折成 0。
#[test]
fn invalid_array_extents_are_structurally_rejected() {
    let negative = TypeExtent::Unary(Box::new(rusty_valkyrie::compiler::syntax::oak::ast::term_nodes::TermUnaryNode {
        operator: ValkyrieTokenType::Minus,
        base: int_literal("3"),
        span: oak_span(0),
    }));
    let cases = vec![
        (Some(negative), ArrayExtentFailure::NegativeLength),
        (Some(tuple_extent(vec![int_literal("2"), int_literal("0")])), ArrayExtentFailure::ZeroDimension),
        (Some(tuple_extent(Vec::new())), ArrayExtentFailure::UnsupportedDimensionCount { count: 0 }),
        (
            Some(tuple_extent(vec![int_literal("1"), int_literal("2"), int_literal("3"), int_literal("4")])),
            ArrayExtentFailure::UnsupportedDimensionCount { count: 4 },
        ),
        (Some(TypeExtent::FloatLiteral { value: "1.5".into(), span: oak_span(3) }), ArrayExtentFailure::NotLiteral),
    ];
    for (extent, expected) in cases {
        let error = lower_type_expression(&bracket_type("i32", extent)).expect_err("非法 extent 必须拒绝");
        assert!(
            matches!(error, HirError::InvalidArrayExtent { ref failure, .. } if failure == &expected),
            "error={error:?}"
        );
    }
}
