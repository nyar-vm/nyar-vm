use rusty_valkyrie::{
    types::hir::{GenericType, HirKind, ValkyrieType},
    *,
};

use rusty_valkyrie::compiler::hir::control_flow_validation::control_flow_value_type_compatible;

#[test]
fn rejects_unbound_names_as_builtin_or_generic_types() {
    for (name, builtin) in [
        ("bool", ValkyrieType::Boolean),
        ("usize", ValkyrieType::Integer32 { signed: true }),
        ("T", ValkyrieType::Utf8),
        ("Self", ValkyrieType::SelfType),
    ] {
        let named = ValkyrieType::Named(Identifier::new(name));
        assert!(!control_flow_value_type_compatible(&builtin, &named));
        assert!(!control_flow_value_type_compatible(&named, &builtin));
    }
    let generic = ValkyrieType::Generic(GenericType { binder: None, name: Identifier::new("T"), kind: HirKind::Type, bounds: Vec::new() });
    assert!(!control_flow_value_type_compatible(&generic, &ValkyrieType::Named(Identifier::new("T"))));
}

#[test]
fn rejects_named_array_and_generic_argument_wildcards() {
    let array = ValkyrieType::Array(Box::new(ValkyrieType::Utf8));
    let named = ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new("Array"))), vec![ValkyrieType::Utf8]);
    assert!(!control_flow_value_type_compatible(&array, &named));
    let named_owner = ValkyrieType::Named(Identifier::new("Box"));
    let applied = ValkyrieType::Apply(Box::new(named_owner.clone()), vec![ValkyrieType::Utf8]);
    assert!(!control_flow_value_type_compatible(&named_owner, &applied));
    let fake_generic = ValkyrieType::Apply(Box::new(named_owner), vec![ValkyrieType::Named(Identifier::new("T"))]);
    assert!(!control_flow_value_type_compatible(&fake_generic, &applied));
}

#[test]
fn keeps_array_text_encodings_distinct() {
    let utf8 = ValkyrieType::Array(Box::new(ValkyrieType::Utf8));
    let utf16 = ValkyrieType::Array(Box::new(ValkyrieType::Utf16));
    assert!(control_flow_value_type_compatible(&utf8, &utf8));
    assert!(!control_flow_value_type_compatible(&utf8, &utf16));
    assert!(!control_flow_value_type_compatible(&utf16, &utf8));
}

#[test]
fn keeps_integer_carriers_distinct_by_width_and_signedness() {
    let i32_ty = ValkyrieType::Integer32 { signed: true };
    let i64_ty = ValkyrieType::Integer64 { signed: true };
    let u32_ty = ValkyrieType::Integer32 { signed: false };
    let u64_ty = ValkyrieType::Integer64 { signed: false };
    for ty in [&i32_ty, &i64_ty, &u32_ty, &u64_ty] {
        assert!(control_flow_value_type_compatible(ty, ty), "同一整数档位必须互容: {ty:?}");
    }
    for (expected, actual) in [(&i32_ty, &i64_ty), (&i64_ty, &i32_ty), (&i32_ty, &u32_ty), (&u32_ty, &i32_ty), (&u32_ty, &u64_ty)] {
        assert!(!control_flow_value_type_compatible(expected, actual), "不同宽度或不同符号的整数不得顶替: {expected:?} vs {actual:?}");
    }
}

#[test]
fn rejects_let_initializer_with_other_integer_carrier() {
    // `i64` 形参不得静默绑定到 `i32` 声明：位宽不同的整数 carrier 不是同一名义身份。
    let source = "micro narrow(value: i64) -> i32 { let narrowed: i32 = value return 0 }";
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("i64 值绑定到 i32 声明必须结构化失败");
    let HirError::LetInitializerTypeMismatch { actual, expected, source, span } = &error
    else {
        panic!("期望 HirError::LetInitializerTypeMismatch，got {error:?}");
    };
    assert_eq!(expected, "i32", "声明类型必须原样进入诊断: {expected}");
    assert_eq!(actual, "i64", "初始化表达式类型必须原样进入诊断: {actual}");
    assert_eq!(source.id, "<control_flow>", "诊断来源必须是控制流校验: {source:?}");
    assert_ne!((span.start, span.end), (0, 0), "诊断 span 必须指向初始化表达式");
}

#[test]
fn rejects_let_initializer_with_signedness_change() {
    // `u32` 形参不得静默绑定到 `i32` 声明：同宽度但符号不同的整数 carrier 不是同一名义身份。
    let source = "micro narrow(value: u32) -> i32 { let narrowed: i32 = value return 0 }";
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("u32 值绑定到 i32 声明必须结构化失败");
    assert!(matches!(error, HirError::LetInitializerTypeMismatch { .. }), "error={error:?}");
}

#[test]
fn keeps_same_integer_carrier_and_integer_literal_let_bindings() {
    // 同一整数名义身份直接通过；整数字面量的宽度由声明类型决定。
    let same = "micro keep(value: i32) -> i32 { let copied: i32 = value return copied }";
    ValkyrieCompiler::default().compile_source(same).unwrap_or_else(|error| panic!("同宽同符号整数绑定必须通过: {error}"));
    for declared in ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"] {
        let source = format!("micro literal() -> {declared} {{ let bound: {declared} = 1 return bound }}");
        ValkyrieCompiler::default()
            .compile_source(&source)
            .unwrap_or_else(|error| panic!("整数字面量必须按声明类型 `{declared}` 取值: {error}"));
    }
}

#[test]
fn accepts_assignment_to_mutable_let_binding() {
    // 规范 definitions.md：`let mut counter = 0; counter = 1` 是合法改值。
    let source = "micro bump() -> i32 { let mut counter: i32 = 0 counter = 1 return counter }";
    ValkyrieCompiler::default().compile_source(source).unwrap_or_else(|error| panic!("`let mut` 绑定赋值必须通过: {error}"));
}

#[test]
fn rejects_assignment_to_immutable_let_with_initializer() {
    // 规范 definitions.md：`let x = 10; x = 20` 是「无法修改不可变变量」的编译错误。
    let source = "micro shadow() -> i32 { let x: i32 = 10 x = 20 return x }";
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("不可变 `let`（带初始化）改值必须结构化失败");
    let HirError::AssignToImmutable { name, source, span } = &error
    else {
        panic!("期望 HirError::AssignToImmutable，got {error:?}");
    };
    assert_eq!(name, "x", "被拒绝的绑定名必须原样进入诊断: {name}");
    assert_eq!(source.id, "<control_flow>", "诊断来源必须是控制流校验: {source:?}");
    assert_ne!((span.start, span.end), (0, 0), "诊断 span 必须指向赋值表达式");
}

#[test]
fn accepts_deferred_initialization_assignment_of_immutable_let() {
    // 规范 definitions.md 延迟初始化例：`let result: i32` 无初始化，分支内首次赋值是绑定不是改值。
    let source = "micro lazy(flag: bool) -> i32 { let result: i32; if flag { result = 1 } else { result = 2 } return result }";
    ValkyrieCompiler::default().compile_source(source).unwrap_or_else(|error| panic!("延迟初始化 `let` 的首次赋值必须通过: {error}"));
}

#[test]
fn registers_parameter_mutability_without_tightening_parameter_rebinding() {
    // `mut` 形参（scope.md 控制符）赋值必须通过。
    let mutable = "micro bump(mut n: i32) -> i32 { n = 1 return n }";
    ValkyrieCompiler::default().compile_source(mutable).unwrap_or_else(|error| panic!("`mut` 形参赋值必须通过: {error}"));
    // 刻意宽松守卫：规范对非 `mut` 形参的名字重绑定沉默（只读承诺针对原地修改），
    // 本轮只登记可变性事实、不收紧；若未来决定收紧形参，此断言须随规范取证一起更新。
    let plain = "micro bump(n: i32) -> i32 { n = 1 return n }";
    ValkyrieCompiler::default().compile_source(plain).unwrap_or_else(|error| panic!("非 `mut` 形参重绑定本轮不得拒绝: {error}"));
}
