//! `imply` 成员方法在宿主泛型参数上调用带 `where` 约束的兄弟方法。
//!
//! `imply Table<K, V>` 的泛型环境没有任何 `where`，成员 `get` 调用
//! `find_slot(self, key: K) where K: Equal, K: Hash` 时，`K` 仍是调用方的
//! 刚性泛型参数。约束 evidence 只能在实例化时按具体 `K` 履行，因此解析阶段
//! 必须接受该调用，而不是把它降成 `<computed>`。

use rusty_valkyrie::{
    ValkyrieCompiler,
    types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirStatementKind, ValkyrieType},
};

use crate::primitives::SourceFixtureCompile;

fn impl_method<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module
        .impls
        .iter()
        .flat_map(|implementation| implementation.methods.iter())
        .find(|method| method.name.as_str() == name)
        .unwrap_or_else(|| panic!("imply method `{name}` 必须出现在 HIR"))
}

fn first_statement_expr(method: &HirFunction) -> &HirExpr {
    let from_statements = method.body.statements.iter().find_map(|statement| match &statement.kind {
        HirStatementKind::Let { initializer: Some(initializer), .. } => Some(initializer.as_ref()),
        HirStatementKind::Expr(expression) => Some(expression.as_ref()),
        _ => None,
    });
    from_statements.or(method.body.expr.as_deref()).expect("方法体必须包含调用语句")
}

#[test]
fn impl_method_resolves_bounded_sibling_on_open_generic_target() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_fixture_source(
            r#"
trait Equal { micro equal(self, other: Self): bool }
trait Hash { micro hash(self): usize }
class Table<K, V> { }
imply Table<K, V> {
    micro get(self, key: K): bool {
        return self.find_slot(key)
    }
    micro find_slot(self, key: K): bool where K: Equal, K: Hash {
        return true
    }
}
"#,
        )
        .expect("开放泛型目标上的 where 义务必须由实例化履行，不能在解析阶段拒绝");

    let get = impl_method(&module, "get");
    let HirExprKind::Return(Some(call)) = &first_statement_expr(get).kind
    else {
        panic!("get 必须返回 find_slot 的调用结果")
    };
    let HirExprKind::Call { resolved: Some(resolved), .. } = &call.kind
    else {
        panic!("`self.find_slot(key)` 必须形成已解析调用合同")
    };
    assert_eq!(resolved.symbol.to_string(), "Table.find_slot");
    assert_eq!(resolved.return_type, ValkyrieType::Boolean);

    // 约束目标必须携带 `imply Table<K, V>` 冻结出的 binder；残留无 binder 的 `Generic`
    // 会让调用点代入与 evidence 查找永远对不上。
    let find_slot = impl_method(&module, "find_slot");
    let target = &find_slot.where_constraints[0].target;
    assert!(
        matches!(target, ValkyrieType::Generic(generic) if generic.binder.is_some()),
        "imply 方法的 where 目标必须绑定 owner 泛型参数，got {target:?}"
    );
}

#[test]
fn generic_field_projection_substitutes_declaration_arguments() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_fixture_source(
            r#"
trait Equal { infix `==`(self, rhs: Self): bool }
structure Entry<K, V> {
    key: K
    value: V
    hash: usize
}
class Table<K, V> { }
imply Table<K, V> {
    micro same(self, key: K, entry: Entry<K, V>): bool where K: Equal {
        return entry.key == key
    }
}
"#,
        )
        .expect("泛型实例的成员字段类型必须代入声明实参，否则 `K: Equal` 的 evidence 对不上");

    let same = impl_method(&module, "same");
    let HirExprKind::Return(Some(comparison)) = &first_statement_expr(same).kind
    else {
        panic!("same 必须返回相等比较结果")
    };
    let HirExprKind::Call { resolved: Some(resolved), .. } = &comparison.kind
    else {
        panic!("`entry.key == key` 必须解析到 `Equal.infix ==`，got {:?}", comparison.kind)
    };
    assert_eq!(resolved.symbol.to_string(), "Equal.infix ==");
}

#[test]
fn concrete_super_trait_evidence_closes_where_witness() {
    let compiler = ValkyrieCompiler::default();
    compiler
        .compile_fixture_source(
            r#"
trait Base { micro base(self) -> i64 }
trait Tagged: Base { micro tag(self) -> i64 }
class Thing { value: i64 }
imply Thing {
    micro new(value: i64): Self {
        return Thing { value: value }
    }
}
imply Thing: Base {
    micro base(self) -> i64 {
        return self.value
    }
}
imply Thing: Tagged {
    micro tag(self) -> i64 {
        return self.value
    }
}
micro run<X>(item: X) -> i64 where X: Tagged {
    return item.tag()
}
[export]
micro entry() -> i64 {
    let seed: i64 = 3
    let thing: Thing = Thing.new(seed)
    return run(thing)
}
"#,
        )
        .expect("具体实参 `Thing` 同时有 `Tagged` 与 super trait `Base` 的显式实现时，where witness 必须闭合");
}

#[test]
fn concrete_super_trait_without_base_impl_stays_rejected() {
    let compiler = ValkyrieCompiler::default();
    assert!(
        compiler
            .compile_fixture_source(
                r#"
trait Base { micro base(self) -> i64 }
trait Tagged: Base { micro tag(self) -> i64 }
class Bare { value: i64 }
imply Bare {
    micro new(value: i64): Self {
        return Bare { value: value }
    }
}
imply Bare: Tagged {
    micro tag(self) -> i64 {
        return self.value
    }
}
micro run<X>(item: X) -> i64 where X: Tagged {
    return item.tag()
}
[export]
micro entry() -> i64 {
    let seed: i64 = 3
    let bare: Bare = Bare.new(seed)
    return run(bare)
}
"#,
            )
            .is_err(),
        "缺少 super trait `Base` 的显式实现时，`Tagged` witness 不得铸造"
    );
}

#[test]
fn impl_member_signature_carries_owner_binder_without_call_site_seeding() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_fixture_source(
            r#"
trait Equal { micro equal(self, other: Self): bool }
class Heap<T> { _value: T }
imply Heap<T> {
    micro sift(self, value: T) -> T where T: Equal {
        return value
    }
}
"#,
        )
        .expect("impl 成员的 owner binder 必须在声明冻结期写定");

    let HirModule { impls, .. } = &module;
    let implementation = &impls[0];
    let ValkyrieType::Nominal { declaration, .. } = &implementation.target
    else {
        panic!("impl target 必须冻结为名义身份，got {:?}", implementation.target)
    };
    let declaration = *declaration;
    let sift = impl_method(&module, "sift");

    let owner_binder = |ty: &ValkyrieType| match ty {
        ValkyrieType::Generic(generic) => generic.binder,
        other => panic!("成员签名里的 `T` 必须冻结为泛型身份，got {other:?}"),
    };
    for param in &sift.params {
        if let ValkyrieType::Generic(generic) = &param.ty {
            assert_eq!(generic.binder.map(|binder| binder.owner), Some(declaration), "形参 owner binder 必须指向 `Heap`");
        }
    }
    let return_binder = owner_binder(&sift.return_type).expect("返回类型 `T` 必须有 binder");
    assert_eq!(return_binder.owner, declaration, "返回类型 owner binder 必须指向 `Heap`");
    let where_binder = owner_binder(&sift.where_constraints[0].target).expect("where 目标 `T` 必须有 binder");
    assert_eq!(where_binder.owner, declaration, "where 目标 owner binder 必须指向 `Heap`");
    assert_eq!(where_binder.ordinal, 0, "`Heap` 的首个泛型参数 ordinal 为 0");
    assert!(
        !format!("{:?}", sift.params).contains("binder: None"),
        "impl 成员路径上不得再出现 binder-less 泛型"
    );
}

#[test]
fn trait_member_where_target_carries_trait_declaration_binder() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_fixture_source(
            r#"
trait Equal { micro equal(self, other: Self): bool }
trait Pool<T> {
    micro take(self) -> T where T: Equal
}
"#,
        )
        .expect("trait 成员的 owner binder 必须在声明冻结期写定");

    let pool = module.traits.iter().find(|item| item.name.as_str() == "Pool").expect("`Pool` 声明");
    let declaration = pool.declaration.expect("trait 声明身份");
    let take = pool.methods.first().expect("`Pool.take` 方法");
    let ValkyrieType::Generic(generic) = &take.where_constraints[0].target
    else {
        panic!("trait 成员 where 目标必须是泛型身份，got {:?}", take.where_constraints[0].target)
    };
    let binder = generic.binder.expect("trait 成员 where 目标必须有 binder");
    assert_eq!(binder.owner, declaration, "trait 成员 binder 必须指向 trait 声明");
}

#[test]
fn unresolvable_impl_member_generic_names_declaration_and_member() {
    let compiler = ValkyrieCompiler::default();
    let error = compiler
        .compile_fixture_source(
            r#"
class Box<T> { _value: T }
imply Box<U> {
    micro bad(self, value: U) -> U {
        return value
    }
}
"#,
        )
        .expect_err("impl 成员里无法冻结的泛型必须结构化失败，不得留下 binder-less 形态");

    let rendered = error.to_string();
    assert!(rendered.contains("Box"), "诊断必须点名 owner 声明，got {rendered}");
    assert!(rendered.contains("bad"), "诊断必须点名成员，got {rendered}");
    assert!(rendered.contains('U'), "诊断必须点名未绑定泛型，got {rendered}");
}

#[test]
fn labeled_tuple_associated_type_freezes_owner_generic_binder() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_fixture_source(
            r#"
trait Carrier { type Item; micro probe(self): i64 }
structure Holder<T> { _value: T }
imply Holder<T>: Carrier {
    type Item = (ordinal: usize, value: T);
    micro probe(self): i64 {
        return 0
    }
}
"#,
        )
        .expect("关联类型是命名元组时，冻结传播必须与冻结检测同样下钻到字段类型");

    let implementation = &module.impls[0];
    let ValkyrieType::Nominal { declaration, .. } = &implementation.target
    else {
        panic!("impl target 必须冻结为名义身份，got {:?}", implementation.target)
    };
    let declaration = *declaration;
    let item = implementation
        .associated_type_impls
        .iter()
        .find(|item| item.name.as_str() == "Item")
        .expect("关联类型 `Item` 必须保留在 impl 里");
    let ValkyrieType::LabeledTuple(fields) = &item.concrete_type
    else {
        panic!("关联类型 `Item` 必须仍是命名元组，got {:?}", item.concrete_type)
    };
    let value = fields.iter().find(|field| field.name.as_str() == "value").expect("命名元组必须有 `value` 字段");
    let ValkyrieType::Generic(generic) = &value.ty
    else {
        panic!("`value` 字段类型必须冻结为泛型身份，got {:?}", value.ty)
    };
    let binder = generic.binder.expect("命名元组字段内的 owner 泛型必须有 binder");
    assert_eq!(binder.owner, declaration, "命名元组字段 binder 必须指向 `Holder` 声明");
    assert_eq!(binder.ordinal, 0, "`Holder` 的首个泛型参数 ordinal 为 0");
}

#[test]
fn labeled_tuple_associated_type_with_unfrozen_generic_still_rejected() {
    let compiler = ValkyrieCompiler::default();
    let error = compiler
        .compile_fixture_source(
            r#"
trait Carrier { type Item; micro probe(self): i64 }
structure Holder<T> { _value: T }
imply Holder<U>: Carrier {
    type Item = (ordinal: usize, value: U);
    micro probe(self): i64 {
        return 0
    }
}
"#,
        )
        .expect_err("命名元组字段内无法冻结的泛型仍须结构化失败，不得静默放行");

    let rendered = error.to_string();
    assert!(rendered.contains("Item"), "诊断必须点名关联类型，got {rendered}");
    assert!(rendered.contains('U'), "诊断必须点名未绑定泛型，got {rendered}");
}

#[test]
fn concrete_target_without_evidence_is_still_rejected() {
    let compiler = ValkyrieCompiler::default();
    assert!(
        compiler
            .compile_fixture_source(
                r#"
trait Equal { micro equal(self, other: Self): bool }
trait Hash { micro hash(self): usize }
class Table<K, V> { }
imply Table<K, V> {
    micro find_slot(self, key: K): bool where K: Equal, K: Hash {
        return true
    }
}
micro caller(table: Table<bool, bool>, key: bool): bool {
    return table.find_slot(key)
}
"#,
            )
            .is_err(),
        "具体类型缺少 Equal/Hash evidence 时不得解析成功"
    );
}
