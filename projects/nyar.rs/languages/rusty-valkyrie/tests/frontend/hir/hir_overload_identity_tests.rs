//! 重载身份合同测试（公开 API 面）。
//!
//! 其余历史用例访问 `hir::overload` 的 pub(crate) 内部函数（架构迁移遗留），
//! 维持既有排除状态整体保留在不编译的 `legacy_internals` 模块内。

use rusty_valkyrie::{
    Identifier, SourceID, ValkyrieCompiler,
    compiler::hir::overload::collect_module_candidates,
    types::hir::{HirExprKind, HirStatementKind, types::ValkyrieType},
};

#[test]
fn chained_generic_trait_result_resolves_declared_member() {
    let compiler = ValkyrieCompiler::default();
    let root = rusty_valkyrie::compiler::syntax::oak::parse_source(
        r#"
class Parcel<T> { value: T }
imply Parcel<T> { micro take(self): T { return self.value } }
trait Cursor { type Item; micro next(self): Parcel<Item> }
micro collect<I, T>(iter: I): T where I: Cursor<Item = T> { return iter.next().take() }
"#,
    )
    .expect("解析链式调用");
    let hir = compiler.lower_root(&root).expect("绑定链式调用");
    compiler.validate_hir_semantic_contract(&hir).expect("链式调用必须绑定完整声明合同");
    let function = hir.functions.iter().find(|function| function.name.as_str() == "collect").unwrap();
    let HirExprKind::Return(Some(result)) = &function.body.expr.as_ref().unwrap().kind
    else {
        panic!("返回调用")
    };
    let HirExprKind::Call { callee, resolved: Some(call), .. } = &result.kind
    else {
        panic!("一次成员调用")
    };
    assert!(matches!(callee.kind, HirExprKind::FieldAccess { .. }));
    assert!(call.declaration.is_some());
    assert_eq!(call.return_type, function.return_type);
}

#[test]
fn generic_aggregate_constructor_provides_field_context_to_nullary_variant() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 5230 });
    let hir = compiler
        .compile_source(
            r#"
unite Option<T> {
    Some { value: T },
    None,
}

class Promise<T> { value: Option<T>, done: bool, cancelled: bool }

trait Future {
    micro poll(self): bool
}

imply Promise<T>: Future {
    micro new(): Self {
        return Promise { value: None, done: false, cancelled: false }
    }
}
"#,
        )
        .expect("generic aggregate constructor should resolve its field context");
    assert!(hir.structs.iter().any(|item| item.name == Identifier::new("Promise")));
}

#[test]
fn qualified_type_static_call_does_not_fall_back_to_unrelated_new() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 5202 });
    let result = compiler.compile_source(
        r#"
[primitive("std::primitive::i64")]
structure i64 { }

structure HashMap<K, V> {
    capacity: i64
}

structure TuiRuntime {
    capacity: i64
}

imply HashMap<K, V> {
    micro new(capacity: i64): Self {
        return HashMap { capacity: capacity }
    }
}

imply TuiRuntime {
    micro new(capacity: i64): Self {
        return TuiRuntime { capacity: capacity }
    }
}

micro main() {
    let map: HashMap<i64, i64> = HashMap::new(0)
}
"#,
    );
    let hir = result.expect("qualified static call must resolve from its declared signature");
    let main = hir.functions.iter().find(|function| function.name.as_str() == "main").expect("main");
    let resolved = main
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            HirStatementKind::Let { initializer: Some(value), .. } => match &value.kind {
                HirExprKind::Call { resolved: Some(resolved), .. } => Some(resolved),
                _ => None,
            },
            _ => None,
        })
        .expect("HashMap::new must carry a resolved call contract");
    assert_eq!(resolved.symbol.to_string(), "HashMap.new");
    // 身份合同：`i64` 由 `[primitive]` 声明身份，形参类型折叠为内建整型载体；
    // 本用例锁定的是限定静态调用绑定声明签名这一身份事实。
    assert_eq!(resolved.parameter_types, vec![ValkyrieType::Integer64 { signed: true }]);
}

/// 同一 owner 的 inherent 成员与 trait impl 成员同形并存时，调用解析按方法解析的
/// 既定语义由 inherent 胜出，不得让同形对撞退化成「无一候选」。
///
/// 野外形态：std 的 `u64` 同时声明 `imply u64 { infix == }` 与 `imply u64: Equal`，
/// `a == b` 必须解析到 inherent 成员；`imply u64: Equal` 的 `!=` 体内 `!(self == rhs)`
/// 也必须解析（Equal 的 `==` 候选与 inherent 候选同形并存）。
#[test]
fn inherent_and_trait_impl_members_with_identical_signature_prefer_inherent() {
    let std = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

imply bool {
    prefix `!`(self): bool {
        if self {
            return false
        }
        return true
    }
}

namespace std.operator;

trait Equal {
    infix `==`(self, other: Self): bool

    infix `!=`(self, other: Self): bool
}

namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

imply u64 {
    infix `==`(self, rhs: Self): bool {
        __u64_eq(self, rhs)
    }
}

imply u64: std::operator::Equal {
    infix `==`(self, rhs: Self): bool {
        __u64_eq(self, rhs)
    }
    infix `!=`(self, rhs: Self): bool {
        return !(self == rhs)
    }
}

[host_contract]
private micro __u64_eq(a: u64, b: u64): bool
"#;
    let app = r#"
namespace demo;

[export]
micro probe(a: u64, b: u64): bool {
    return a != b
}
"#;
    compile_operator_groups(std, app).expect("inherent 与 trait 同形成员并存时必须由 inherent 胜出");
}

/// 某成员只经 trait impl 提供（无 inherent 同名兄弟）时，具体接收者上的调用必须
/// 解析到该 impl 成员——trait 归属事实不构成匹配门槛。
#[test]
fn trait_impl_member_resolves_when_no_inherent_sibling_exists() {
    let std = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

imply bool {
    prefix `!`(self): bool {
        if self {
            return false
        }
        return true
    }
}

namespace std.operator;

trait Equal {
    infix `==`(self, other: Self): bool

    infix `!=`(self, other: Self): bool
}

trait PartialOrder {
    infix `<`(self, other: Self): bool

    infix `<=`(self, other: Self): bool
}

namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

imply u64 {
    infix `==`(self, rhs: Self): bool {
        __u64_eq(self, rhs)
    }
}

imply u64: std::operator::Equal {
    infix `==`(self, rhs: Self): bool {
        __u64_eq(self, rhs)
    }
    infix `!=`(self, rhs: Self): bool {
        return !(self == rhs)
    }
}

imply u64: std::operator::PartialOrder {
    infix `<`(self, rhs: Self): bool {
        __u64_lt(self, rhs)
    }
    infix `<=`(self, rhs: Self): bool {
        return !(rhs < self)
    }
}

[host_contract]
private micro __u64_eq(a: u64, b: u64): bool

[host_contract]
private micro __u64_lt(a: u64, b: u64): bool
"#;
    let app = r#"
namespace demo;

[export]
micro probe(a: u64, b: u64): bool {
    return a < b
}
"#;
    compile_operator_groups(std, app).expect("`a < b` 必须解析到 `PartialOrder` 的 impl 成员");
}

fn compile_operator_groups(std: &str, app: &str) -> Result<(), String> {
    use rusty_valkyrie::{CompilerSourceGroup, compiler::diagnostics::pending, hir::lowering::DEFAULT_COMPILE_ARCH};
    pending::clear_pending_diagnostics();
    let groups = crate::primitives::groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", std),
        CompilerSourceGroup::single_source("app", "app", app).as_entry().with_direct_dependencies(vec!["std".into()]),
    ]);
    let result = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH);
    let collected = pending::take_pending_diagnostics();
    match result {
        Ok(_) if collected.total() == 0 => Ok(()),
        Ok(_) => Err(format!("编译成功但诊断汇非空: {collected:?}")),
        Err(error) => Err(format!("{error}; diagnostics: {collected:?}")),
    }
}

/// 内建 `[T]` 的 owner 是 canonical `std.collection.Array` 的声明身份，不是
/// 「本地任意名叫 `Array` 的单泛型聚合」。用户在同一项目里声明自己的
/// `Array<T>` 时，它必须保持普通名义身份，不被当成内建数组 owner。
#[test]
fn same_named_user_array_keeps_nominal_identity_next_to_builtin_owner() {
    let source = r#"
namespace app;

class Array<T> {
    value: T
}

namespace std.collection;

class Array<T> {
}

[export]
micro keep_std(a: Array<i32>): bool {
    return false
}

namespace app;

[export]
micro wrap(a: Array<i32>): Array<i32> {
    return a
}

[export]
micro probe(items: [i32]): bool {
    return false
}
"#;
    let hir = ValkyrieCompiler::default()
        .compile_source(&crate::primitives::with_primitives(source))
        .expect("同名 Array 不得导致绑定失败");
    let wrap = hir.functions.iter().find(|function| function.name.as_str() == "wrap").unwrap();
    assert!(
        matches!(wrap.params[0].ty, ValkyrieType::Nominal { .. }),
        "用户自己的 `Array<T>` 必须冻结为名义声明身份，实际: {:?}",
        wrap.params[0].ty
    );
    let keep_std = hir.functions.iter().find(|function| function.name.as_str() == "keep_std").unwrap();
    assert!(
        matches!(keep_std.params[0].ty, ValkyrieType::Array(_)),
        "canonical `std.collection.Array<T>` 必须折叠为内建 `[T]`，实际: {:?}",
        keep_std.params[0].ty
    );
    let probe = hir.functions.iter().find(|function| function.name.as_str() == "probe").unwrap();
    assert!(matches!(probe.params[0].ty, ValkyrieType::Array(_)), "内建 `[T]` 仍由 canonical owner 承担");
}

/// 无同名冲突的标准场景：`Array<T>` 解析到可见的 `std.collection.Array`，
/// 仍按内建 `[T]` 结构化身份冻结。
#[test]
fn canonical_array_owner_still_folds_array_spelling() {
    let std = r#"
namespace std.collection;

class Array<T> {
    _typing: Phantom<T>
}

structure Phantom<T> {
}
"#;
    let app = r#"
namespace app;

[export]
micro wrap(a: Array<i32>): Array<i32> {
    return a
}
"#;
    let modules = compile_groups(std, app).expect("标准 Array 场景必须编译");
    let app_module = modules
        .iter()
        .find(|module| module.functions.iter().any(|function| function.name.as_str() == "wrap"))
        .expect("app 模块");
    let wrap = app_module.functions.iter().find(|function| function.name.as_str() == "wrap").unwrap();
    assert!(matches!(wrap.params[0].ty, ValkyrieType::Array(_)), "`Array<T>` 必须折叠为内建 `[T]`");
}

/// 守卫：数组 owner 的类型身份只允许 owner-freeze 折叠这一条路径产生。
/// 构造器候选与固有方法候选的签名里一旦出现 `Nominal{declaration: Array}` 或裸 `Named("Array")`，
/// 就是"第二身份生产者"复活，必须当缺陷失败。
#[test]
fn array_owner_candidates_never_forge_a_second_identity() {
    let source = r#"
namespace std.primitive;

[primitive("std::primitive::i32")] structure i32 { }
[primitive("std::primitive::u64")] structure u64 { }
[primitive("std::primitive::bool")] structure bool { }

type usize = u64;

namespace std.collection;

class Array<T> {
    _typing: T
}

imply Array<T> {
    micro probe(self) -> bool { return true }
}
"#;
    let hir = ValkyrieCompiler::default().compile_source(source).expect("数组 owner 绑定");
    let array_owner =
        hir.structs.iter().find(|item| item.name.as_str() == "Array").expect("Array 声明").constructor_declaration.expect("Array 声明身份");
    let candidates = collect_module_candidates(&hir);
    let mut structural_hits = 0usize;
    for candidate in &candidates {
        for ty in std::iter::once(&candidate.signature.return_type).chain(candidate.signature.params.iter()) {
            if matches!(ty, ValkyrieType::Named(name) if name.as_str() == "Array") {
                panic!("候选 {:?} 以裸 `Named(Array)` 表示数组 owner: {ty:?}", candidate.symbol);
            }
            if let ValkyrieType::Nominal { declaration, .. } = ty {
                assert_ne!(*declaration, array_owner, "候选 {:?} 以 `Nominal` 铸出数组 owner 的第二身份: {ty:?}", candidate.symbol);
            }
            if matches!(ty, ValkyrieType::Array(_)) {
                structural_hits += 1;
            }
        }
    }
    assert!(structural_hits >= 2, "数组 owner 的构造器与方法候选必须都产出结构形态，实际命中 {structural_hits}");
}

/// 可见作用域里多个 `Array<T>` 声明且没有 canonical `std.collection.Array` 时，
/// 必须结构化报错；不得像旧实现那样 `.first()` 静默取首项。
#[test]
fn multiple_array_owners_are_reported_as_structured_error() {
    let source = r#"
namespace a;

class Array<T> {
    value: T
}

namespace b;

class Array<T> {
    value: T
}

micro probe(items: [i32]): bool {
    return false
}
"#;
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("多候选内建数组 owner 必须报错");
    assert!(error.to_string().contains("ambiguous"), "必须是结构化歧义错误，实际: {error}");
}

/// 对照：`class Array<T>`（泛型 owner）的成员已迁到 `imply`，因为泛型 owner
/// 的实例闭合入口尚未覆盖声明体成员；单态 owner 的声明体方法不需要迁移，
/// 它的候选绑定类自身的 `Nominal` 身份而不是裸名。
#[test]
fn monomorphic_class_body_method_candidate_binds_its_nominal_owner() {
    let source = r#"
namespace std.primitive;

[primitive("std::primitive::i32")] structure i32 { }

namespace app;

class Counter {
    micro bump(self) -> i32 { return 0 }
}
"#;
    let hir = ValkyrieCompiler::default().compile_source(source).expect("单态类体方法必须完成 HIR");
    let owner = hir
        .structs
        .iter()
        .find(|item| item.name.as_str() == "Counter")
        .expect("Counter 声明")
        .constructor_declaration
        .expect("Counter 声明身份");
    let candidate = collect_module_candidates(&hir)
        .into_iter()
        .find(|candidate| candidate.symbol.parts().last().is_some_and(|part| part.as_str() == "bump"))
        .expect("类体方法必须产生自己的候选");
    assert!(candidate.instance.is_some(), "单态 owner 的候选必须冻结实例身份");
    match candidate.impl_target.as_ref() {
        Some(ValkyrieType::Nominal { declaration, .. }) => {
            assert_eq!(*declaration, owner, "候选的 impl 目标必须是类自身的 `Nominal` 身份")
        }
        other => panic!("候选的 impl 目标必须是 `Nominal`，实际 {other:?}"),
    }
}

fn compile_groups(std: &str, app: &str) -> Result<Vec<rusty_valkyrie::types::hir::HirModule>, String> {
    use rusty_valkyrie::{CompilerSourceGroup, compiler::diagnostics::pending, hir::lowering::DEFAULT_COMPILE_ARCH};
    pending::clear_pending_diagnostics();
    let groups = crate::primitives::groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std.collection", "std.collection", std),
        CompilerSourceGroup::single_source("app", "app", app).as_entry().with_direct_dependencies(vec!["std.collection".into()]),
    ]);
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).map_err(|error| error.to_string())
}

#[cfg(false)] // 历史用例访问 overload pub(crate) 内部函数（架构迁移遗留），维持排除
mod legacy_internals {
    use rusty_valkyrie::{
        hir::{
            overload::{OverloadCandidate, OverloadDomain, OverloadMatchKind, OverloadResolutionError, resolve_overload},
            type_relation::TypeRelationContext,
        },
        mir::AggregateFieldIndex,
        types::hir::{
            HirCallArgument, HirCallableDomain, HirExpr, HirExprKind, HirIdentifier, HirModule, HirResolvedCall, HirStatementKind,
            HirTraitBound, HirWhereConstraint,
            types::{HirParam, ValkyrieType},
        },
        *,
    };
    use std::collections::{BTreeMap, BTreeSet};

    use rusty_valkyrie::compiler::hir::overload::{
        extract_callable_name, infer_expr_type, infer_scrutinee_type, match_call_candidate, match_singleton_method_candidate,
        substitute_associated_equations, substitute_type_self, try_resolve_constructor, try_resolve_nullary_constructor,
    };

    fn test_binder(owner: u32, ordinal: u32) -> rusty_valkyrie::compiler::types::hir::GenericBinderId {
        rusty_valkyrie::compiler::types::hir::GenericBinderId { owner: nyar_types::ItemId::from_index(owner).unwrap(), ordinal }
    }

    fn test_generic(name: &str) -> ValkyrieType {
        ValkyrieType::Generic(rusty_valkyrie::compiler::types::hir::GenericType {
            binder: Some(test_binder(900, 0)),
            name: Identifier::new(name),
            kind: rusty_valkyrie::compiler::types::hir::HirKind::Type,
            bounds: vec![],
        })
    }

    #[test]
    fn nominal_arguments_preserve_identity_and_substitute_associated_self() {
        let declaration = nyar_types::ItemId::from_index(41).unwrap();
        let subject = test_generic("I");
        let item = Identifier::new("Item");
        let original = ValkyrieType::Nominal {
            declaration,
            arguments: vec![ValkyrieType::Associated(Box::new(rusty_valkyrie::compiler::types::hir::AssociatedType {
                base: ValkyrieType::SelfType,
                name: item.clone(),
                type_arguments: vec![],
            }))],
        };
        let bound = substitute_type_self(&original, &subject);
        let equations = BTreeMap::from([(item, ValkyrieType::Utf8)]);
        assert_eq!(
            substitute_associated_equations(&bound, &subject, &equations),
            ValkyrieType::Nominal { declaration, arguments: vec![ValkyrieType::Utf8] }
        );
    }

    #[test]
    fn nested_call_type_requires_the_selected_contract() {
        let compiler = ValkyrieCompiler::default();
        let mut hir = compiler
            .compile_source(
                "micro produce() -> i64 { 7 } micro consume(value: i64) -> i64 { value } micro caller() -> i64 { consume(produce()) }",
            )
            .expect("嵌套调用必须从源码完成声明选择");
        compiler.validate_hir_semantic_contract(&hir).expect("完整调用合同必须通过");
        let caller = hir.functions.iter_mut().find(|function| function.name.as_str() == "caller").unwrap();
        let HirExprKind::Call { args, .. } = &mut caller.body.expr.as_mut().unwrap().kind
        else {
            panic!("预期外层调用");
        };
        let nested = &mut args[0].value;
        // 该源码没有声明数值 primitive：`i64` 无声明身份，按 `Named` 冻结，不能折成内建标量
        // `Integer64`。内建标量形式只属于字面量推断与显式数值转换目标。
        let expected = Some(ValkyrieType::Named(Identifier::new("i64")));
        assert_eq!(infer_expr_type(nested, &BTreeMap::new()), expected);
        assert_eq!(infer_scrutinee_type(nested, &BTreeMap::new(), &AggregateFieldIndex::default(), &BTreeSet::new()), expected);
        let HirExprKind::Call { resolved, .. } = &mut nested.kind
        else {
            panic!("预期嵌套调用");
        };
        assert!(resolved.take().is_some());
        assert_eq!(infer_expr_type(nested, &BTreeMap::new()), None);
        assert_eq!(infer_scrutinee_type(nested, &BTreeMap::new(), &AggregateFieldIndex::default(), &BTreeSet::new()), None);
        assert!(compiler.validate_hir_semantic_contract(&hir).is_err());
    }

    #[test]
    fn panic_and_format_names_require_source_callable_declarations() {
        let compiler = ValkyrieCompiler::default();
        for source in ["micro caller(value: i64) -> i64 { panic(value) }", "micro formatter(value: i64) -> i64 { format(value) }"] {
            let error = compiler.compile_source(source).expect_err("未声明名称不得获得合成 callable 合同");
            assert!(error.to_string().contains("unresolved call"), "应在 Semantic HIR 合同边界失败：{error}");
        }

        let hir = compiler
            .compile_source(
                r#"
micro panic(value: i64) -> i64 { value }
micro caller(value: i64) -> i64 { panic(value) }
"#,
            )
            .expect("显式声明应通过普通 callable resolver");
        let declaration = hir.functions.iter().find(|function| function.name.as_str() == "panic").unwrap();
        let caller = hir.functions.iter().find(|function| function.name.as_str() == "caller").unwrap();
        let HirExprKind::Call { resolved: Some(call), .. } = &caller.body.expr.as_ref().unwrap().kind
        else {
            panic!("显式声明必须得到正式调用合同");
        };
        assert_eq!(call.instance, declaration.instance);
    }

    #[test]
    fn construction_type_requires_the_selected_contract() {
        let compiler = ValkyrieCompiler::default();
        let mut hir = compiler
            .compile_source("structure Parcel { value: i64 } micro create() -> Parcel { Parcel { value: 7 } }")
            .expect("构造必须从源码完成字段绑定与声明选择");
        compiler.validate_hir_semantic_contract(&hir).expect("完整构造合同必须通过");
        let parcel_declaration = hir.structs[0].constructor_declaration.expect("结构声明身份");
        let create = hir.functions.iter_mut().find(|function| function.name.as_str() == "create").unwrap();
        let construction = create.body.expr.as_mut().unwrap();
        let expected = Some(ValkyrieType::Nominal { declaration: parcel_declaration, arguments: vec![] });
        assert_eq!(infer_expr_type(construction, &BTreeMap::new()), expected);
        assert_eq!(infer_scrutinee_type(construction, &BTreeMap::new(), &AggregateFieldIndex::default(), &BTreeSet::new()), expected);
        let HirExprKind::Construct { resolved, .. } = &mut construction.kind
        else {
            panic!("预期结构构造");
        };
        assert!(resolved.take().is_some());
        assert_eq!(infer_expr_type(construction, &BTreeMap::new()), None);
        assert_eq!(infer_scrutinee_type(construction, &BTreeMap::new(), &AggregateFieldIndex::default(), &BTreeSet::new()), None);
        assert!(compiler.validate_hir_semantic_contract(&hir).is_err());
    }

    #[test]
    fn selected_overload_preserves_all_declaration_facts_in_any_order() {
        let selected = declaration_candidate();
        let mut other = selected.clone();
        other.owner = Some(Identifier::new("OtherOwner"));
        other.trait_owner = Some(nyar_types::ItemId::from_index(910).unwrap());
        other.param_specs.clear();
        other.generic_binders.clear();
        other.where_constraints.clear();
        other.match_kind = OverloadMatchKind::Trait;

        for candidates in [vec![other.clone(), selected.clone()], vec![selected.clone(), other]] {
            assert_eq!(resolve_overload(&candidates).unwrap(), selected);
        }
    }

    #[test]
    fn equal_rank_declarations_cannot_merge_after_erasing_facts() {
        let declaration = declaration_candidate();
        let mut different_owner = declaration.clone();
        different_owner.owner = Some(Identifier::new("OtherOwner"));
        let mut different_trait = declaration.clone();
        different_trait.trait_owner = Some(nyar_types::ItemId::from_index(910).unwrap());
        let mut different_receiver = declaration.clone();
        different_receiver.param_specs[0].name.name = Identifier::new("value");
        let mut different_binders = declaration.clone();
        different_binders.generic_binders.insert(test_binder(901, 0));
        let mut different_constraints = declaration.clone();
        different_constraints.where_constraints[0].bounds[0].trait_path = path("OtherBound");

        for (fact, other) in [
            ("owner", different_owner),
            ("trait_owner", different_trait),
            ("receiver", different_receiver),
            ("generic_binders", different_binders),
            ("where_constraints", different_constraints),
        ] {
            for candidates in [vec![declaration.clone(), other.clone()], vec![other, declaration.clone()]] {
                assert_eq!(
                    resolve_overload(&candidates),
                    Err(OverloadResolutionError::Ambiguous { candidates: vec![path("apply"), path("apply")] }),
                    "{fact}"
                );
            }
        }
    }

    #[test]
    fn repeated_complete_candidate_preserves_the_declaration() {
        let declaration = declaration_candidate();
        assert_eq!(resolve_overload(&[declaration.clone(), declaration.clone()]).unwrap(), declaration);
    }

    #[test]
    fn empty_overload_set_fails_without_a_default_candidate() {
        assert_eq!(resolve_overload(&[]), Err(OverloadResolutionError::NoMatch));
    }

    #[test]
    fn singleton_matching_preserves_the_complete_declared_signature() {
        let declaration = declaration_candidate();
        let matched = match_singleton_method_candidate(
            &declaration,
            &[],
            &TypeRelationContext::from_module(&HirModule::default()),
            &BTreeMap::new(),
            &AggregateFieldIndex::default(),
            &BTreeSet::new(),
            true,
        )
        .expect("singleton 匹配不得擦除 self 或声明合同");
        assert_eq!(matched, declaration);
    }

    #[test]
    fn singleton_matching_rejects_an_argument_without_a_type_fact() {
        let declaration = declaration_candidate();
        let span = rusty_valkyrie::compiler::types::SourceSpan::new(SourceID::default(), 0, 0);
        let unknown = HirCallArgument::positional(HirExpr {
            kind: HirExprKind::Variable(HirIdentifier { name: Identifier::new("unknown"), shadow_index: 0, span: span.clone() }),
            span,
        });
        assert!(
            match_singleton_method_candidate(
                &declaration,
                &[unknown],
                &TypeRelationContext::from_module(&HirModule::default()),
                &BTreeMap::new(),
                &AggregateFieldIndex::default(),
                &BTreeSet::new(),
                true,
            )
            .is_none()
        );
    }

    fn declaration_candidate() -> OverloadCandidate {
        let mut declaration = candidate("apply", OverloadMatchKind::NominalExact);
        declaration.owner = Some(Identifier::new("Owner"));
        declaration.trait_owner = Some(nyar_types::ItemId::from_index(911).unwrap());
        let mut receiver = HirParam::default();
        receiver.name.name = Identifier::new("self");
        receiver.ty = ValkyrieType::Named(Identifier::new("Owner"));
        declaration.signature.params = vec![receiver.ty.clone()];
        declaration.where_constraints.push(HirWhereConstraint {
            target: ValkyrieType::Named(Identifier::new("Element")),
            bounds: vec![HirTraitBound { declaration: None, trait_path: path("Bound"), type_arguments: vec![], associated_types: vec![] }],
            span: receiver.name.span.clone(),
        });
        declaration.param_specs = vec![receiver];
        declaration.generic_binders.insert(test_binder(900, 0));
        declaration
    }

    fn candidate(name: &str, match_kind: OverloadMatchKind) -> OverloadCandidate {
        OverloadCandidate::new(path(name), OverloadDomain::Function, vec![], ValkyrieType::Unit, match_kind)
    }

    fn path(name: &str) -> NamePath {
        NamePath::new(vec![Identifier::new(name)])
    }

    fn resolve_constructor_contract(candidates: &[OverloadCandidate], actual: &[Option<ValkyrieType>]) -> Option<HirResolvedCall> {
        let mut locals = BTreeMap::new();
        let arguments = actual
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                let name = Identifier::new(&format!("argument_{index}"));
                let span = rusty_valkyrie::compiler::types::SourceSpan::new(SourceID::default(), 0, 0);
                if let Some(ty) = ty {
                    locals.insert(name.to_string(), ty.clone());
                }
                HirExpr { kind: HirExprKind::Variable(HirIdentifier { name, shadow_index: 0, span: span.clone() }), span }
            })
            .collect::<Vec<_>>();
        try_resolve_constructor(
            &NamePath::new(vec![Identifier::new("Present")]),
            &[],
            &arguments,
            candidates,
            &TypeRelationContext::from_module(&HirModule::default()),
            &locals,
            &AggregateFieldIndex::default(),
            &BTreeSet::new(),
            None,
        )
    }

    fn generic_constructor(owner: &str, parameters: Vec<ValkyrieType>) -> OverloadCandidate {
        OverloadCandidate::new_method(
            Identifier::new(owner),
            NamePath::new(vec![Identifier::new("Present")]),
            OverloadDomain::Constructor,
            parameters,
            ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new(owner))), vec![test_generic("T")]),
            OverloadMatchKind::Row,
        )
        .with_generic_binder(test_binder(900, 0))
    }

    #[test]
    fn constructor_contract_instantiates_parameters_and_return_with_declared_owner() {
        let candidate = generic_constructor("Envelope", vec![test_generic("T")]);
        let resolved = resolve_constructor_contract(&[candidate], &[Some(ValkyrieType::Utf8)]).expect("声明 binder 应完成代入");
        assert_eq!(resolved.symbol, NamePath::new(vec![Identifier::new("Envelope"), Identifier::new("Present")]));
        assert_eq!(resolved.parameter_types, vec![ValkyrieType::Utf8]);
        assert_eq!(
            resolved.return_type,
            ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new("Envelope"))), vec![ValkyrieType::Utf8])
        );
        assert!(!resolved.has_receiver);
    }

    #[test]
    fn constructor_contract_rejects_conflicting_repeated_binder() {
        let parameter = test_generic("T");
        let candidate = generic_constructor("Envelope", vec![parameter.clone(), parameter]);
        assert!(resolve_constructor_contract(&[candidate], &[Some(ValkyrieType::Utf8), Some(ValkyrieType::Boolean)]).is_none());
    }

    #[test]
    fn constructor_contract_rejects_missing_argument_type_and_ambiguous_owner() {
        let parameter = test_generic("T");
        let candidates = vec![generic_constructor("Alpha", vec![parameter.clone()]), generic_constructor("Beta", vec![parameter])];
        assert!(resolve_constructor_contract(&candidates[..1], &[None]).is_none());
        assert!(resolve_constructor_contract(&candidates, &[None]).is_none());
        assert!(resolve_constructor_contract(&candidates, &[Some(ValkyrieType::Utf8)]).is_none());
    }

    #[test]
    fn constructor_contract_rejects_partial_sum_identity_facts() {
        let mut candidate = generic_constructor("Envelope", vec![test_generic("T")]);
        candidate.sum_owner = nyar_types::ItemId::from_index(1);
        candidate.sum_variant = None;
        assert!(resolve_constructor_contract(&[candidate], &[Some(ValkyrieType::Utf8)]).is_none());
    }

    #[test]
    fn option_none_turbofish_call_is_resolved_in_hir() {
        let source = r#"
namespace test;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
micro take(flag: bool) -> Option<i64> {
    if flag {
        return option_none::<i64>()
    }
    let value: i64 = 1
    return Some(value)
}
"#;
        let root = rusty_valkyrie::compiler::syntax::oak::parse_source(source).expect("parse option_none fixture");
        let take_ast = root
            .items
            .iter()
            .filter_map(|item| match item {
                rusty_valkyrie::compiler::syntax::oak::ast::StatementNode::Micro(micro) if micro.name.name == "take" => Some(micro),
                _ => None,
            })
            .next()
            .expect("take micro in ast");
        let if_stmt = take_ast.body.statements.iter().find_map(|statement| match statement {
            rusty_valkyrie::compiler::syntax::oak::ast::Statement::Expression(expr)
                if matches!(expr.expr, rusty_valkyrie::compiler::syntax::oak::ast::TermExpression::If { .. }) =>
            {
                Some(())
            }
            _ => None,
        });
        let ast_stmt_kinds = take_ast
            .body
            .statements
            .iter()
            .map(|statement| match statement {
                rusty_valkyrie::compiler::syntax::oak::ast::Statement::Let(_) => "let".to_string(),
                rusty_valkyrie::compiler::syntax::oak::ast::Statement::Expression(expr) => match &expr.expr {
                    rusty_valkyrie::compiler::syntax::oak::ast::TermExpression::If { .. } => "if-expr".to_string(),
                    rusty_valkyrie::compiler::syntax::oak::ast::TermExpression::Return(_) => "return-expr".to_string(),
                    other => format!("expr({:?})", std::mem::discriminant(other)),
                },
                other => format!("other({:?})", std::mem::discriminant(other)),
            })
            .collect::<Vec<_>>();
        assert!(if_stmt.is_some(), "parser must preserve if statement before option_none turbofish call, got {ast_stmt_kinds:?}");
        let hir = rusty_valkyrie::ValkyrieCompiler::default().compile_source(source).expect("option_none hir compile");
        let take = hir.functions.iter().find(|function| function.name.as_str() == "take").expect("take");
        let mut call_debug = Vec::new();
        fn walk_calls(expr: &HirExpr, out: &mut Vec<String>) {
            match &expr.kind {
                HirExprKind::Call { callee, resolved, .. } => {
                    let name = extract_callable_name(callee).map(|id| id.to_string()).unwrap_or_else(|| format!("{:?}", callee.kind));
                    let resolved = resolved
                        .as_ref()
                        .map(|call| {
                            format!("{} domain={:?} sum={}/{}", call.symbol, call.domain, call.sum_owner.is_some(), call.sum_variant.is_some())
                        })
                        .unwrap_or_else(|| "unresolved".to_string());
                    out.push(format!("{name} => {resolved}"));
                    if let HirExprKind::GenericApply { callee, .. } = &callee.kind {
                        walk_calls(callee, out);
                    }
                }
                HirExprKind::Return(Some(inner)) | HirExprKind::Yield(Some(inner)) => walk_calls(inner, out),
                HirExprKind::If { condition, then_branch, else_branch, .. } => {
                    walk_calls(condition, out);
                    if let Some(expr) = &then_branch.expr {
                        walk_calls(expr, out);
                    }
                    for statement in &then_branch.statements {
                        if let HirStatementKind::Expr(expr) = &statement.kind {
                            walk_calls(expr, out);
                        }
                    }
                    if let Some(else_branch) = else_branch {
                        if let Some(expr) = &else_branch.expr {
                            walk_calls(expr, out);
                        }
                        for statement in &else_branch.statements {
                            if let HirStatementKind::Expr(expr) = &statement.kind {
                                walk_calls(expr, out);
                            }
                        }
                    }
                }
                HirExprKind::Block(block) => {
                    if let Some(expr) = &block.expr {
                        walk_calls(expr, out);
                    }
                    for statement in &block.statements {
                        if let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind {
                            walk_calls(initializer, out);
                        }
                        if let HirStatementKind::Expr(expr) = &statement.kind {
                            walk_calls(expr, out);
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(expr) = &take.body.expr {
            walk_calls(expr, &mut call_debug);
        }
        for statement in &take.body.statements {
            if let HirStatementKind::Expr(expr) = &statement.kind {
                walk_calls(expr, &mut call_debug);
            }
            if let HirStatementKind::Let { initializer: Some(initializer), .. } = &statement.kind {
                walk_calls(initializer, &mut call_debug);
            }
        }
        let if_shape = take.body.statements.first().map(|statement| format!("{:?}", statement.kind));
        let option_none_call = call_debug.iter().find(|line| line.starts_with("option_none"));
        assert!(
            option_none_call.is_some(),
            "expected option_none call in take body, got calls={call_debug:?} stmts={} tail={} first_stmt={if_shape:?}",
            take.body.statements.len(),
            take.body.expr.is_some()
        );
        let line = option_none_call.expect("option_none line");
        assert!(!line.contains("unresolved"), "option_none must resolve: {line}");
        assert!(line.contains("domain=Function"), "option_none must stay a normal function call: {line}");
        assert!(!line.contains("sum=true/true"), "option_none must not carry sum constructor facts: {line}");
    }

    #[test]
    fn turbofish_generic_call_substitutes_return_type_and_keeps_callable_instance() {
        let hir = rusty_valkyrie::ValkyrieCompiler::default()
            .compile_source(
                r#"
namespace test;
unite Option<T> {
    Some { value: T },
    None,
}
micro option_none<T>() -> Option<T> {
    let empty: Option<T> = None
    return empty
}
micro pick(flag: bool) -> Option<i64> {
    if flag {
        return option_none::<i64>()
    }
    let value: i64 = 1
    return Some(value)
}
"#,
            )
            .expect("turbofish generic call compile");
        let pick = hir.functions.iter().find(|function| function.name.as_str() == "pick").expect("pick function");
        let mut resolved_call = None;
        fn walk_for_option_none_call(expr: &HirExpr, resolved_call: &mut Option<HirResolvedCall>) {
            match &expr.kind {
                HirExprKind::Call { callee, resolved, .. }
                    if extract_callable_name(callee).is_some_and(|name| name.as_str() == "option_none") =>
                {
                    *resolved_call = resolved.clone();
                }
                HirExprKind::Return(Some(inner)) | HirExprKind::Yield(Some(inner)) => walk_for_option_none_call(inner, resolved_call),
                HirExprKind::If { condition, then_branch, else_branch, .. } => {
                    walk_for_option_none_call(condition, resolved_call);
                    if let Some(expr) = &then_branch.expr {
                        walk_for_option_none_call(expr, resolved_call);
                    }
                    for statement in &then_branch.statements {
                        if let HirStatementKind::Expr(expr) = &statement.kind {
                            walk_for_option_none_call(expr, resolved_call);
                        }
                    }
                    if let Some(else_branch) = else_branch {
                        if let Some(expr) = &else_branch.expr {
                            walk_for_option_none_call(expr, resolved_call);
                        }
                        for statement in &else_branch.statements {
                            if let HirStatementKind::Expr(expr) = &statement.kind {
                                walk_for_option_none_call(expr, resolved_call);
                            }
                        }
                    }
                }
                HirExprKind::Block(block) => {
                    if let Some(expr) = &block.expr {
                        walk_for_option_none_call(expr, resolved_call);
                    }
                    for statement in &block.statements {
                        if let HirStatementKind::Expr(expr) = &statement.kind {
                            walk_for_option_none_call(expr, resolved_call);
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(expr) = &pick.body.expr {
            walk_for_option_none_call(expr, &mut resolved_call);
        }
        for statement in &pick.body.statements {
            if let HirStatementKind::Expr(expr) = &statement.kind {
                walk_for_option_none_call(expr, &mut resolved_call);
            }
        }
        let resolved = resolved_call.expect("pick must contain resolved option_none turbofish call");
        assert_eq!(resolved.domain, HirCallableDomain::Function);
        assert!(resolved.instance.is_some(), "option_none turbofish call must keep callable instance");
        assert!(resolved.declaration.is_some(), "option_none turbofish call must keep declaration identity");
        assert!(resolved.sum_owner.is_none(), "option_none must not be rewritten into None constructor");
        assert!(resolved.sum_variant.is_none(), "option_none must not be rewritten into None constructor");
        assert_eq!(
            resolved.return_type,
            ValkyrieType::Nominal {
                declaration: hir.enums[0].declaration.expect("sum 声明身份"),
                arguments: vec![ValkyrieType::Named(Identifier::new("i64"))],
            }
        );
    }

    #[test]
    fn resolves_array_subscript_on_generic_heap_array_param() {
        let source = r#"
micro get<T>(array: [T], ordinal: i64): T {
    return array::[ordinal]
}
"#;
        rusty_valkyrie::ValkyrieCompiler::default().compile_source(source).expect("generic heap array subscript");
    }

    fn test_sum_nominal(declaration_index: u32, arguments: Vec<ValkyrieType>) -> ValkyrieType {
        ValkyrieType::Nominal { declaration: nyar_types::ItemId::from_index(declaration_index).expect("test ItemId"), arguments }
    }

    #[test]
    fn nullary_none_resolution_uses_return_type_owner_context() {
        let option_none = nullary_variant_constructor("Option", "None", 1);
        let other_none = nullary_variant_constructor("Other", "None", 99);
        let contextual = test_sum_nominal(1, vec![ValkyrieType::Integer32 { signed: true }]);
        let resolved = try_resolve_nullary_constructor(
            &Identifier::new("None"),
            Some(&contextual),
            &[option_none, other_none],
            &TypeRelationContext::from_module(&HirModule::default()),
            &BTreeMap::new(),
            &AggregateFieldIndex::default(),
            &BTreeSet::new(),
        )
        .expect("return Nominal Option<i32> should close Option::None");
        assert_eq!(resolved.sum_owner, nyar_types::ItemId::from_index(1));
        assert!(resolved.sum_variant.is_some());
        assert_eq!(resolved.domain, HirCallableDomain::Constructor);
    }

    fn nullary_variant_constructor(owner: &str, variant: &str, sum_owner_index: u32) -> OverloadCandidate {
        let sum_owner = nyar_types::ItemId::from_index(sum_owner_index).expect("test sum_owner");
        let mut candidate = OverloadCandidate::new_method(
            Identifier::new(owner),
            NamePath::new(vec![Identifier::new(variant)]),
            OverloadDomain::Constructor,
            Vec::new(),
            test_sum_nominal(sum_owner_index, vec![test_generic("T")]),
            OverloadMatchKind::Row,
        )
        .with_generic_binder(test_binder(900, 0));
        candidate.sum_owner = Some(sum_owner);
        candidate.sum_variant = nyar_types::ItemId::from_index(sum_owner_index + 1);
        candidate
    }

    #[test]
    fn constructor_contract_does_not_substitute_nominal_parameter_as_generic() {
        let candidate = generic_constructor("Envelope", vec![ValkyrieType::Named(Identifier::new("DeclaredPayload"))]);
        assert!(resolve_constructor_contract(&[candidate], &[Some(ValkyrieType::Utf8)]).is_none());
    }

    #[test]
    fn constructor_contract_from_source_preserves_generic_sum_signature() {
        let hir = ValkyrieCompiler::new(SourceID { version_id: 5210 })
            .compile_source(
                r#"
unite Envelope<T> {
    Present { value: T },
    Absent,
}
micro wrap(value: utf8) -> Envelope<utf8> {
    let wrapped = Present { value: value }
    return wrapped
}
"#,
            )
            .expect("当前源码中的构造声明应完成实例化");
        let function = hir.functions.iter().find(|function| function.name.as_str() == "wrap").expect("wrap");
        let HirStatementKind::Let { initializer: Some(initializer), .. } = &function.body.statements[0].kind
        else {
            panic!("缺少构造表达式");
        };
        let HirExprKind::Construct { resolved: Some(resolved), .. } = &initializer.kind
        else {
            panic!("构造身份未解析：{initializer:?}");
        };
        assert_eq!(resolved.symbol, NamePath::new(vec![Identifier::new("Envelope"), Identifier::new("Present")]));
        assert_eq!(resolved.parameter_types, vec![ValkyrieType::Utf8]);
        let envelope = hir.enums.iter().find(|item| item.name.as_str() == "Envelope").expect("Envelope");
        let declaration = envelope.declaration.expect("Envelope declaration");
        assert_eq!(resolved.return_type, ValkyrieType::Nominal { declaration, arguments: vec![ValkyrieType::Utf8] });
    }

    #[test]
    fn ambiguous_bare_new_does_not_resolve_via_simple_name_fallback() {
        let compiler = ValkyrieCompiler::new(SourceID { version_id: 5201 });
        let hir = compiler.compile_source(
            r#"
structure Alpha { value: i64 }

structure Beta { value: i64 }

imply Alpha {
    micro new(value: i64): Self {
        return Alpha { value: value }
    }
}

imply Beta {
    micro new(value: i64): Self {
        return Beta { value: value }
    }
}

micro main() {
    let x = new(1)
}
"#,
        );

        match hir {
            Ok(hir) => {
                let main = hir.functions.iter().find(|function| function.name.as_str() == "main").expect("main");
                let mut saw_bare_new = false;
                for statement in &main.body.statements {
                    visit_calls_looking_for_bare_new(&statement.kind, &mut saw_bare_new);
                }
                assert!(saw_bare_new, "expected bare `new` call in main");
            }
            Err(error) => {
                let text = error.to_string();
                assert!(
                    text.contains("new") || text.contains("SMIR003") || text.contains("unresolved") || text.contains("ambiguous"),
                    "ambiguous bare `new` must fail closed without inventing a binding, got {text}"
                );
            }
        }
    }

    fn visit_calls_looking_for_bare_new(kind: &HirStatementKind, saw_bare_new: &mut bool) {
        let expression = match kind {
            HirStatementKind::Let { initializer: Some(value), .. } | HirStatementKind::Expr(value) => value.as_ref(),
            _ => return,
        };
        if let HirExprKind::Call { callee, resolved, .. } = &expression.kind {
            if extract_callable_name(callee).is_some_and(|name| name.as_str() == "new") {
                *saw_bare_new = true;
                assert!(resolved.is_none(), "ambiguous bare `new` must stay unresolved, got {resolved:?}");
            }
        }
    }

    #[test]
    fn receiver_presence_is_derived_from_the_declared_self_parameter() {
        let compiler = ValkyrieCompiler::new(SourceID { version_id: 5203 });
        let hir = compiler
            .compile_source(
                r#"
structure Counter { value: i64 }

imply Counter {
    micro read(self): i64 { self.value }
    micro make(value: i64): i64 { value }
}

micro main(counter: Counter) {
    let current = counter.read()
    let created = Counter.make(1)
}
"#,
            )
            .expect("declared instance and static methods must resolve");
        let main = hir.functions.iter().find(|function| function.name.as_str() == "main").expect("main");
        let resolved = main
            .body
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                HirStatementKind::Let { initializer: Some(value), .. } => match &value.kind {
                    HirExprKind::Call { resolved: Some(resolved), .. } => Some(resolved),
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].symbol.to_string(), "Counter.read");
        assert_eq!(
            resolved[0].parameter_types,
            vec![ValkyrieType::Nominal {
                declaration: hir.structs[0].constructor_declaration.expect("receiver 声明身份"), arguments: vec![]
            }]
        );
        assert!(resolved[0].has_receiver);
        assert_eq!(resolved[1].symbol.to_string(), "Counter.make");
        assert_eq!(resolved[1].parameter_types, vec![ValkyrieType::Named(Identifier::new("i64"))]);
        assert!(!resolved[1].has_receiver);
    }

    #[test]
    fn instance_syntax_cannot_supply_a_missing_self_parameter() {
        let compiler = ValkyrieCompiler::new(SourceID { version_id: 5204 });
        let error = compiler
            .compile_source(
                r#"
structure Counter { }

imply Counter {
    micro make(value: i64): i64 { value }
}

micro main(counter: Counter) {
    counter.make(1)
}
"#,
            )
            .expect_err("a static method must not gain a receiver from dot syntax");
        let message = error.to_string();
        assert!(message.contains("SMIR003") || message.contains("unresolved"), "unexpected failure: {message}");
    }
}

/// `Utf8Builder` 自己的 `append(char)` / `append(&Utf8Text)` 与集合
/// `LiteralSequence<T>::append(T)` 同名时，成员解析必须先按接收者声明身份过滤，
/// 再对剩余候选做实参特异性排序；`text.append(char)` 必须唯一命中 `append(char)`。
#[test]
fn member_call_filters_candidates_by_receiver_identity_before_specificity() {
    let std = r#"
namespace std.primitive;

[primitive("std::primitive::u8")]
structure u8 { }

[primitive("std::primitive::usize")]
structure usize { }

namespace std.text;

[primitive("std::text::char")]
structure char { }

class Utf8Text {
}

imply Utf8Text {
    suffix `⁅ ⁆`(self, offset: usize): char {
        return 'a'
    }

    micro from_bytes(bytes: [u8]): Self {
        return Utf8Text { }
    }
}

class Utf8Builder {
    bytes: ArrayList<u8>
}

imply Utf8Builder {
    micro append(mut self, c: char): unit {
    }

    micro append(mut self, text: &Utf8Text): unit {
    }
}

namespace std.collection;

trait LiteralSequence<T> {
    micro append(mut self, value: T): unit
}

class ArrayList<T> {
    capacity: usize
}

imply ArrayList<T>: LiteralSequence<T> {
    micro append(mut self, value: T): unit {
    }
}

class LinkedList<T> {
    size: usize
}

imply LinkedList<T>: LiteralSequence<T> {
    micro append(mut self, value: T): unit {
    }
}
"#;
    let direct = r#"
namespace app;

[export]
micro probe(text: Utf8Builder, c: char): unit {
    text.append(c)
}
"#;
    let indexed = r#"
namespace app;

[export]
micro probe(text: Utf8Builder, s: Utf8Text, at: usize): unit {
    text.append(s⁅at⁆)
}
"#;
    let by_value = r#"
namespace app;

using std.collection;

[export]
micro probe(text: Utf8Builder, s: Utf8Text): unit {
    text.append(s)
}
"#;
    let constructed = r#"
namespace app;

using std.collection;

[export]
micro probe(text: Utf8Builder): unit {
    text.append(Utf8Text::from_bytes([104, 101]))
}
"#;
    compile_operator_groups(std, direct).expect("`Utf8Builder.append(char)` 必须唯一命中");
    compile_operator_groups(std, indexed).expect("下标取出的 `char` 必须解析到 `Utf8Builder.append(char)`");
    compile_operator_groups(std, by_value).expect("`Utf8Text` 值实参必须解析到 `Utf8Builder.append(&Utf8Text)`");
    compile_operator_groups(std, constructed).expect("构造实参必须解析到 `Utf8Builder.append(&Utf8Text)`");
}

/// 反例：接收者类型未知时不得静默选中第一个候选，诊断必须点名候选集。
#[test]
fn member_call_on_unknown_receiver_reports_named_candidates() {
    let source = r#"
namespace app;

[primitive("std::text::char")]
structure char { }

class Utf8Builder {
}

imply Utf8Builder {
    micro append(self, c: char): unit {
    }
}

class Bag {
}

imply Bag {
    micro append(self, c: char): unit {
    }
}

class Holder {
}

micro probe(holder: Holder, c: char): unit {
    holder.missing.append(c)
}
"#;
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("未知接收者的成员调用必须结构化失败");
    let message = error.to_string();
    assert!(message.contains("receiver type is unknown"), "必须报接收者类型未定，实际: {message}");
    assert!(message.contains("`append`") && message.contains("2 candidate(s)"), "诊断必须点名 `append` 候选集，实际: {message}");
}

/// 字段写入的值按「目标字段声明类型」解析，而不是外层表达式类型：
/// `self.node = None` 必须解析成 `Option<K>` 的无载荷变体，过 SMIR006 合同校验。
#[test]
fn store_field_resolves_value_against_declared_field_type() {
    let source = r#"
namespace app;

unite Option<T> {
    Some { value: T },
    None
}

class Holder<K> {
    node: Option<K>
}

imply Holder<K> {
    micro new(node: Option<K>): Self {
        return Self { node: node }
    }

    micro clear(mut self): unit {
        self.node = None
    }
}
"#;
    let mir = ValkyrieCompiler::default()
        .compile_source_to_mir(source)
        .expect("字段写入必须按字段声明类型解析 `None`，不得落到 SMIR006");
    rusty_valkyrie::mir::validate_semantic_module(&mir).expect("StoreField 值类型必须与字段声明一致");
}

/// 反例：字段写入的值确实与声明类型不符时，SMIR006 仍须结构化报错。
#[test]
fn store_field_mismatch_still_reports_smir006() {
    let source = r#"
namespace app;

class Holder {
    count: i64
}

imply Holder {
    micro new(count: i64): Self {
        return Self { count: count }
    }

    micro bad(mut self, flag: bool): unit {
        self.count = flag
    }
}
"#;
    let error = ValkyrieCompiler::default()
        .compile_source_to_mir(&crate::primitives::with_primitives(source))
        .expect_err("字段写入值类型不符必须报结构化失败");
    assert!(error.to_string().contains("SMIR006"), "必须复用 SMIR006 合同码，实际: {error}");
}

const NESTED_CONSTRUCT_SOURCE: &str = r#"
namespace app;

class Inner<T> {
    capacity: i64
}

imply Inner<T> {
    micro new(capacity: i64): Self {
        return Self { capacity: capacity }
    }
}

class Outer<T> {
    inner: Inner<T>
}

imply Outer<T> {
    micro new(capacity: i64): Self {
        return Self { inner: Inner::new(capacity) }
    }

    micro push(mut self, element: Inner<T>): unit {
        self.inner = element
    }
}
"#;

/// 实参位置的期望类型逐槽下传：嵌套 `Inner::new` 的元素类型只能由外层 `push` 形参决定。
#[test]
fn nested_new_element_type_flows_from_argument_context() {
    let source = format!(
        "{NESTED_CONSTRUCT_SOURCE}\nmicro build(capacity: i64): i64 {{\n    let mut outer: Outer<i64> = Outer::new(capacity)\n    outer.push(Inner::new(capacity))\n    return capacity\n}}\n"
    );
    let mir = ValkyrieCompiler::default()
        .compile_source_to_mir(&crate::primitives::with_primitives(&source))
        .expect("嵌套构造的实参必须由形参槽类型定型");
    rusty_valkyrie::mir::validate_semantic_module(&mir).expect("嵌套构造实参类型必须与形参槽一致");
}

/// 反例：实参与形参槽类型确实不符时，仍须结构化报出未解析调用。
#[test]
fn nested_new_rejects_argument_incompatible_with_slot() {
    let source = format!(
        "{NESTED_CONSTRUCT_SOURCE}\nmicro build(capacity: i64): i64 {{\n    let mut outer: Outer<i64> = Outer::new(capacity)\n    outer.push(capacity)\n    return capacity\n}}\n"
    );
    let error = ValkyrieCompiler::default()
        .compile_source_to_mir(&crate::primitives::with_primitives(&source))
        .expect_err("实参与形参槽不符必须结构化失败");
    let message = error.to_string();
    assert!(message.contains("unresolved call `outer.push`"), "必须点名未解析调用，实际: {message}");
    assert!(message.contains("none matches"), "必须说明无一候选匹配，实际: {message}");
}

/// 同一 `imply` 块内同名构造按 arity 区分：零参与一参都必须各自命中自己的候选。
///
/// 回归点：零参候选曾对一参调用「空匹配」（实参绑定结果被 `zip` 静默截断），
/// 与真正匹配的候选并列判 Ambiguous，最终退化成「无一候选」。
#[test]
fn same_owner_constructors_with_distinct_arity_both_resolve() {
    let source = r#"
namespace app;

[primitive("std::primitive::i64")]
structure i64 { }

class Bag<T> {
    capacity: i64
}

imply Bag<T> {
    micro new(): Self {
        return Self { capacity: 0 }
    }

    micro new(capacity: i64): Self {
        return Self { capacity: capacity }
    }
}

micro build(): i64 {
    let zero: Bag<i64> = Bag::new()
    let one: Bag<i64> = Bag::new(7)
    return one.capacity
}
"#;
    let hir = ValkyrieCompiler::default().compile_source(source).expect("两种 arity 的构造都必须解析");
    let build = hir.functions.iter().find(|function| function.name.as_str() == "build").expect("build");
    let resolved = build
        .body
        .statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            HirStatementKind::Let { initializer: Some(value), .. } => match &value.kind {
                HirExprKind::Call { resolved, .. } => resolved.as_ref(),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(resolved.len(), 2, "两次构造都必须带解析结果");
    assert!(resolved[0].parameter_types.is_empty(), "零参构造必须绑定零参签名: {:?}", resolved[0]);
    assert_eq!(
        resolved[1].parameter_types,
        vec![ValkyrieType::Integer64 { signed: true }],
        "一参构造必须绑定 `i64` 形参签名"
    );
}

/// 反例：零参构造收到一参调用时，必须在 HIR 调用边界结构化失败，
/// 不得让空匹配混入候选集后退化成候选歧义，也不得漏到 SMIR 才报 arity。
#[test]
fn nullary_constructor_rejects_extra_argument_at_call_boundary() {
    let source = r#"
namespace app;

[primitive("std::primitive::i64")]
structure i64 { }

class Bag<T> {
    capacity: i64
}

imply Bag<T> {
    micro new(): Self {
        return Self { capacity: 0 }
    }
}

micro build(value: i64): i64 {
    let bag: Bag<i64> = Bag::new(value)
    return bag.capacity
}
"#;
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("零参构造收到实参必须失败");
    assert!(matches!(error, rusty_valkyrie::HirError::UnresolvedCall { .. }), "必须在调用边界报 UnresolvedCall: {error:?}");
    let message = error.to_string();
    assert!(message.contains("`Bag.new`") && message.contains("none matches"), "诊断必须点名候选与匹配失败，实际: {message}");
}
