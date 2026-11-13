//! 消费者包对 `std::primitive` 内建标量的身份解析：注解必须折叠回内建类型身份。
//!
//! `[primitive("std::primitive::f64")] structure f64 { }` 是宿主绑定声明，
//! 不得在注解位置产生第二个名义身份；注解、表达式与 `using` 三条路径必须得到
//! 同一个内建 `ValkyrieType`。

use rusty_valkyrie::{
    CompilerSourceGroup, MirLowerer, MirOperand, MirTerminator, ValkyrieCompiler,
    compiler::emitter::frontend_contract::concretize_type,
    types::{Identifier as HirIdentifier, hir::ValkyrieType},
};

const CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::f64")]
structure f64 { }

[primitive("std::primitive::bool")]
structure bool { }
"#;

const APP: &str = r#"
namespace demo.app;

using std.primitive::{f64};

micro one() -> f64 {
    return 1.0
}

[export]
micro probe_let() -> f64 {
    let value: f64 = 1.0
    return value
}

[main]
micro main() -> f64 {
    return one()
}
"#;

fn app_module() -> rusty_valkyrie::types::hir::HirModule {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.app", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("消费者包必须完成 HIR 解析");
    modules.into_iter().last().expect("消费者包模块")
}

const ANNOTATION_ONLY: &str = r#"
namespace demo.ann;

using std.primitive::{f64};

micro passthrough(value: f64) -> f64 {
    return value
}

micro chain(value: f64) -> f64 {
    return passthrough(value)
}

micro seed() -> f64 {
    return seed()
}

[main]
micro main() -> f64 {
    return chain(seed())
}
"#;

#[test]
fn annotation_only_module_reveals_identity_shape() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.ann", ANNOTATION_ONLY).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native");
    match modules {
        Ok(modules) => {
            let module = modules.into_iter().last().expect("消费者包模块");
            let passthrough = module.functions.iter().find(|function| function.name.as_str() == "passthrough").expect("`passthrough` 声明");
            assert_eq!(passthrough.return_type, ValkyrieType::Float64, "注解身份: {:?}", passthrough.return_type);
        }
        Err(error) => panic!("消费者包必须完成 HIR 解析: {error:?}"),
    }
}

#[test]
fn consumer_primitive_annotation_folds_to_builtin_identity() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.app", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native");
    match modules {
        Ok(modules) => {
            let module = modules.into_iter().last().expect("消费者包模块");
            let one = module.functions.iter().find(|function| function.name.as_str() == "one").expect("`one` 声明");
            assert_eq!(one.return_type, ValkyrieType::Float64, "注解 `f64` 必须解析到内建 `Float64`，不得是 `Nominal`");
        }
        Err(error) => panic!("消费者包必须完成 HIR 解析: {error:?}"),
    }
}

const SAME_MODULE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::f64")]
structure f64 { }

namespace demo.same;

micro one() -> f64 {
    return 1.0
}

[main]
micro main() -> f64 {
    return one()
}
"#;

/// 对照：声明与使用同在一个模块时同样必须折叠到内建身份。
#[test]
fn same_module_primitive_annotation_folds_to_builtin_identity() {
    let groups = vec![CompilerSourceGroup::single_source("app", "demo.same", SAME_MODULE).as_entry()];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("同模块声明必须完成 HIR 解析");
    let module = modules.into_iter().last().expect("同模块");
    let one = module.functions.iter().find(|function| function.name.as_str() == "one").expect("`one` 声明");
    assert_eq!(one.return_type, ValkyrieType::Float64, "同模块注解 `f64` 也必须折叠到内建身份");
}

// ---------- 全标量族：注解 + 表达式 ----------

const SCALAR_PRIMITIVE_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i8")]
structure i8 { }

[primitive("std::primitive::i16")]
structure i16 { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::i128")]
structure i128 { }

[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::u16")]
structure u16 { }

[primitive("std::primitive::u32")]
structure u32 { }

[primitive("std::primitive::u64")]
structure u64 { }

[primitive("std::primitive::u128")]
structure u128 { }

[primitive("std::primitive::f32")]
structure f32 { }

[primitive("std::primitive::f64")]
structure f64 { }

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::unit")]
structure unit { }

[primitive("std::primitive::void")]
structure void { }
"#;

const SCALAR_TEXT_CORE: &str = r#"
namespace std.text;

[primitive("std::text::char")]
structure char { }
"#;

const SCALAR_APP: &str = r#"
namespace demo.scalars;

[export]
micro probe_i8(value: i8) -> i8 {
    return value
}

[export]
micro probe_i16(value: i16) -> i16 {
    return value
}

[export]
micro probe_i32(value: i32) -> i32 {
    return value
}

[export]
micro probe_i64(value: i64) -> i64 {
    return value
}

[export]
micro probe_i128(value: i128) -> i128 {
    return value
}

[export]
micro probe_u8(value: u8) -> u8 {
    return value
}

[export]
micro probe_u16(value: u16) -> u16 {
    return value
}

[export]
micro probe_u32(value: u32) -> u32 {
    return value
}

[export]
micro probe_u64(value: u64) -> u64 {
    return value
}

[export]
micro probe_u128(value: u128) -> u128 {
    return value
}

[export]
micro probe_f32(value: f32) -> f32 {
    return value
}

[export]
micro probe_f64(value: f64) -> f64 {
    return value
}

[export]
micro probe_bool(value: bool) -> bool {
    return value
}

[export]
micro probe_char(value: char) -> char {
    return value
}

[export]
micro probe_unit() -> unit {
    return
}

[export]
micro probe_void() -> void {
}

[main]
micro main() -> unit {
    return
}
"#;

fn scalar_expectations() -> Vec<(&'static str, ValkyrieType)> {
    vec![
        ("probe_i8", ValkyrieType::Integer8 { signed: true }),
        ("probe_i16", ValkyrieType::Integer16 { signed: true }),
        ("probe_i32", ValkyrieType::Integer32 { signed: true }),
        ("probe_i64", ValkyrieType::Integer64 { signed: true }),
        ("probe_i128", ValkyrieType::Integer128 { signed: true }),
        ("probe_u8", ValkyrieType::Integer8 { signed: false }),
        ("probe_u16", ValkyrieType::Integer16 { signed: false }),
        ("probe_u32", ValkyrieType::Integer32 { signed: false }),
        ("probe_u64", ValkyrieType::Integer64 { signed: false }),
        ("probe_u128", ValkyrieType::Integer128 { signed: false }),
        ("probe_f32", ValkyrieType::Float32),
        ("probe_f64", ValkyrieType::Float64),
        ("probe_bool", ValkyrieType::Boolean),
        ("probe_char", ValkyrieType::Character),
        ("probe_unit", ValkyrieType::Unit),
        ("probe_void", ValkyrieType::Void),
    ]
}

fn mir_function<'a>(module: &'a rusty_valkyrie::mir::MirModule, name: &str) -> &'a rusty_valkyrie::MirFunction {
    module
        .functions
        .iter()
        .find(|function| function.symbol.parts().last().map(|part| part.as_str()) == Some(name))
        .unwrap_or_else(|| panic!("MIR 缺少函数 `{name}`，现有: {:?}", module.functions.iter().map(|f| f.display_symbol.clone()).collect::<Vec<_>>()))
}

/// 全标量族：每条都有注解与表达式正例。
///
/// 注解侧（形参/返回）与表达式侧（`return value` 的 SSA 值）必须是同一个内建载体，
/// 且内建载体在类型事实表里各只有一条 intern 记录。
#[test]
fn scalar_family_annotations_and_expressions_fold_to_builtin_identity() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", SCALAR_PRIMITIVE_CORE),
        CompilerSourceGroup::single_source("text", "std.text", SCALAR_TEXT_CORE),
        CompilerSourceGroup::single_source("app", "demo.scalars", SCALAR_APP)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "text".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("全标量族消费者包必须完成 HIR 解析");
    let module = modules.into_iter().last().expect("消费者包模块");
    for (name, expected) in scalar_expectations() {
        let function = module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}`"));
        assert_eq!(&function.return_type, &expected, "`{name}` 返回注解必须折叠到内建身份");
        match expected {
            ValkyrieType::Unit | ValkyrieType::Void => assert!(function.params.is_empty(), "`{name}` 不应有形参"),
            ref expected => {
                assert_eq!(function.params.len(), 1, "`{name}` 必须有单个形参");
                assert_eq!(&function.params[0].ty, expected, "`{name}` 形参注解必须折叠到内建身份");
            }
        }
    }

    // 表达式侧：消费者模块本身不再持有 primitive 声明聚合，类型事实表只应有内建载体。
    let mir = MirLowerer::lower_module_semantic(&module);
    for (name, expected) in scalar_expectations() {
        let function = mir_function(&mir, name);
        assert_eq!(&function.return_type, &expected, "`{name}` MIR 返回类型必须是内建载体");
        match expected {
            ValkyrieType::Unit | ValkyrieType::Void => assert!(function.param_types.is_empty(), "`{name}` MIR 不应有形参"),
            ref expected => assert_eq!(function.param_types, vec![expected.clone()], "`{name}` MIR 形参类型必须是内建载体"),
        }
        // 表达式侧：定位返回值的 SSA 值类型，与注解同身份。
        let entry = function.blocks.iter().find(|block| block.id == function.entry).expect("入口块");
        if let MirTerminator::Return { value: Some(MirOperand::Value(value)) } = &entry.terminator {
            assert_eq!(
                function.value_types.get(value),
                Some(&expected),
                "`{name}` 返回表达式值的类型必须与注解同身份，不得是 `Nominal`"
            );
        }
    }
    assert!(
        !mir.type_identities.keys().any(|ty| ty.primitive_identity().is_some()),
        "消费者包类型事实表不得出现任何 primitive `Nominal`：{:?}",
        mir.type_identities.keys().filter_map(|ty| ty.primitive_identity()).collect::<Vec<_>>()
    );
    for (_, expected) in scalar_expectations() {
        assert!(mir.type_identities.contains_key(&expected), "类型事实表必须为内建载体 `{expected:?}` 分配唯一 intern 记录");
    }
}

// ---------- 同路径 / 跨路径同 TypeId ----------

const PATH_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::f64")]
structure f64 { }

[primitive("std::primitive::i32")]
structure i32 { }
"#;

const PATH_APP_SHORT: &str = r#"
namespace demo.paths;

using std.primitive::{f64};
using std::primitive::{i32};

[export]
micro via_short(value: f64) -> f64 {
    return value
}

[export]
micro via_colon_using(value: i32) -> i32 {
    return value
}

[export]
micro via_qualified(value: std::primitive::f64) -> std::primitive::f64 {
    return value
}

[main]
micro main() -> f64 {
    return via_short(1.0)
}
"#;

/// 同一内建标量：短名、点号 `using`、冒号 `using` 与全限定路径必须得到同一个 TypeId。
///
/// TypeId 由类型事实表按内建载体 intern；两条路径若落到不同形态，表里会出现
/// 第二条 key（通常是 `Nominal`），这里用「表里恰好一条 `f64` 且两条函数签名都指向它」锁定。
#[test]
fn short_and_qualified_paths_share_one_interleaved_type_identity() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", PATH_CORE),
        CompilerSourceGroup::single_source("app", "demo.paths", PATH_APP_SHORT).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("路径写法必须完成 HIR 解析");
    let module = modules.into_iter().last().expect("消费者包模块");
    for name in ["via_short", "via_qualified"] {
        let function = module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}`"));
        assert_eq!(function.return_type, ValkyrieType::Float64, "`{name}` 必须解析到内建 `f64`");
        assert_eq!(function.params[0].ty, ValkyrieType::Float64, "`{name}` 形参必须解析到内建 `f64`");
    }
    let colon = module.functions.iter().find(|function| function.name.as_str() == "via_colon_using").expect("`via_colon_using`");
    assert_eq!(colon.return_type, ValkyrieType::Integer32 { signed: true }, "冒号 `using` 必须解析到内建 `i32`");

    let mir = MirLowerer::lower_module_semantic(&module);
    let f64_id = *mir.type_identities.get(&ValkyrieType::Float64).expect("`f64` 必须有唯一 intern 记录");
    for name in ["via_short", "via_qualified"] {
        let function = mir_function(&mir, name);
        assert_eq!(mir.type_identities.get(&function.param_types[0]), Some(&f64_id), "`{name}` 形参必须与短名路径同 TypeId");
        assert_eq!(mir.type_identities.get(&function.return_type), Some(&f64_id), "`{name}` 返回必须与短名路径同 TypeId");
    }
    assert_eq!(
        mir.type_identities.keys().filter(|ty| ty.primitive_identity() == Some("std::primitive::f64")).count(),
        0,
        "消费者包类型事实表不得持有 `f64` 的 primitive `Nominal`"
    );
}

// ---------- `imply f64` 分发键 ----------

const DISPATCH_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::f64")]
structure f64 { }

imply f64 {
    micro twice(self, rhs: f64): f64 {
        return __f64_add(self, rhs)
    }
}

[host_contract]
private micro __f64_add(lhs: f64, rhs: f64): f64
"#;

const DISPATCH_APP: &str = r#"
namespace demo.dispatch;

using std.primitive::{f64};

[export]
micro probe(a: f64, b: f64) -> f64 {
    return a.twice(b)
}
"#;

/// `imply f64` 的 `Self` owner 折叠后，方法分发键与实参类型必须是同一个内建载体。
#[test]
fn imply_f64_dispatch_key_matches_builtin_argument_type() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", DISPATCH_CORE),
        CompilerSourceGroup::single_source("app", "demo.dispatch", DISPATCH_APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("`imply f64` 必须完成 HIR 解析");
    let module = modules.into_iter().last().expect("消费者包模块");
    let probe = module.functions.iter().find(|function| function.name.as_str() == "probe").expect("`probe`");
    let body = probe.body.expr.as_ref().expect("`probe` 必须有返回表达式");
    let rusty_valkyrie::types::hir::HirExprKind::Return(Some(call)) = &body.kind else {
        panic!("`probe` 体必须是返回调用，实际: {:?}", body.kind);
    };
    let rusty_valkyrie::types::hir::HirExprKind::Call { resolved: Some(call), .. } = &call.kind else {
        panic!("`a.twice(b)` 必须解析到 impl 成员");
    };
    assert!(call.symbol.to_string().ends_with("twice"), "分发键必须是 impl 成员: {}", call.symbol);
    assert!(call.has_receiver, "`twice` 必须带接收者");
    assert_eq!(
        call.parameter_types,
        vec![ValkyrieType::Float64, ValkyrieType::Float64],
        "分发键与实参类型必须都是内建 `f64`，不得出现 `Nominal`"
    );
    assert_eq!(call.return_type, ValkyrieType::Float64, "impl 成员返回类型必须是内建 `f64`");
}

// ---------- `[primitive]` 声明不再产生类型位 `Nominal` 的扫描式断言 ----------

const DECLARATION_SCAN: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::u64")]
structure u64 { }

[primitive("std::primitive::f64")]
structure f64 { }

[primitive("std::primitive::bool")]
structure bool { }

micro take_int(value: i32) -> i32 {
    return value
}

micro take_unit(flag: bool) -> bool {
    return flag
}

micro take_float(value: f64) -> f64 {
    return value
}
"#;

/// 扫描：全部 `[primitive]` 声明在**类型位置**（形参、返回、SSA 值）都不再产生 `Nominal`。
///
/// 声明 stub 也不得作为聚合进入类型事实表，由
/// `primitive_declarations_never_produce_nominal_anywhere` 单独锁定。
#[test]
fn primitive_declarations_never_produce_type_position_nominal() {
    let hir = ValkyrieCompiler::default().compile_source(DECLARATION_SCAN).expect("声明扫描夹具必须完成编译");
    let mir = MirLowerer::lower_module_semantic(&hir);
    for function in &mir.functions {
        for ty in function.param_types.iter().chain(std::iter::once(&function.return_type)).chain(function.value_types.values()) {
            assert!(
                !matches!(ty, ValkyrieType::Nominal { primitive_identity: Some(_), .. }),
                "函数 `{}` 的类型位置出现 primitive `Nominal`: {ty:?}",
                function.display_symbol
            );
        }
    }
}

/// 声明登记面的收口断言：类型事实表里不允许残留任何 `[primitive]` 声明键。
///
/// 折叠后的标量 stub 只是宿主绑定载体，没有字段与布局，不得再以 `Nominal` 进入
/// `mir::ssa::type_analysis` 的 `aggregate_type_key` 或 canonical 的聚合登记。
/// 若这里重新出现非空集合，说明 stub 又被当成聚合物化，必须回退到折叠路径。
#[test]
fn primitive_declarations_never_produce_nominal_anywhere() {
    let hir = ValkyrieCompiler::default().compile_source(DECLARATION_SCAN).expect("声明扫描夹具必须完成编译");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let identities = mir
        .type_identities
        .keys()
        .filter_map(|ty| ty.primitive_identity())
        .collect::<Vec<_>>();
    assert!(identities.is_empty(), "类型事实表不得残留 primitive `Nominal` 声明键: {identities:?}");
}

/// `f128` 没有 carrier：按名映射必须失败关闭或保持 `Named`，不得静默降精度到 `f64`。
#[test]
fn f128_name_fails_closed_instead_of_degrading_to_f64() {
    let named = ValkyrieType::Named(HirIdentifier::new("f128"));
    match concretize_type(&named) {
        Ok(nyar_types::NyarType::Float64) => panic!("`f128` 不得映射到 `Float64`（静默降精度）"),
        Ok(nyar_types::NyarType::Named(name)) => assert_eq!(name.as_str(), "f128", "`f128` 只能保持自身命名，不得变成别的载体"),
        Ok(other) => panic!("`f128` 没有 carrier，不允许落到 {other:?}"),
        Err(_) => {}
    }
}

/// `any` / `null` 不进折表：它们是顶/空类型，不是宿主绑定标量。
///
/// 本条锁定它们的现存形态与坐标，防止「静默放过」：
/// - 它们若仍是 `Nominal`，说明 std 仍以 `[primitive] structure` 声明（std 侧待改）；
/// - 它们若消失，说明已引入顶/空类型载体或 std 声明形态已改，本缺口登记必须同步更新。
#[test]
fn any_and_null_are_pinned_as_the_non_scalar_primitive_gap() {
    const ANY_NULL: &str = r#"
namespace std.primitive;

[primitive("std::primitive::any")]
structure `any` { }

[primitive("std::primitive::null")]
structure `null` { }
"#;
    let hir = ValkyrieCompiler::default().compile_source(ANY_NULL).expect("`any`/`null` 声明必须完成编译");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let mut identities = mir
        .type_identities
        .keys()
        .filter_map(|ty| ty.primitive_identity())
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_eq!(
        identities,
        vec!["std::primitive::any", "std::primitive::null"],
        "`any`/`null` 的现存形态已在 std `source/primitive/any.v`、`source/primitive/null.v` 登记；形态变化必须同步缺口"
    );
}
