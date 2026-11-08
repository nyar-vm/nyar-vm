//! 字段访问作为 receiver 的回归，以及继承槽位字段的合成合同。
//!
//! 前一组用例固定 `infer_scrutinee_type` 既有覆盖的字段访问形态：结构体字段、
//! 带标签元组、泛型字段、`self` 字段、调用结果、trait 类型字段等，均不得回退成
//! `receiver type unknown`。
//!
//! 后一组用例固定 `parents` 进入 HIR 字段空间：父类槽位必须是带正式 `FieldId`
//! 的真实字段，且父类槽位按继承声明在前、自身声明字段在后。重名冲突必须结构化
//! 失败，不得静默覆盖。
//!
//! 注意：当前固定的 oak 解析器只接受头部 `class Derived: Base` 形式；`(Base)`
//! 与 `(parent: Base)` 写法在该版本不被识别为父类列表，因此用例以可解析写法固定合同。

use rusty_valkyrie::{hir::*, HirError, SourceID, ValkyrieCompiler};

use crate::primitives::SourceFixtureCompile;

const PRELUDE: &str = r#"
namespace app;

class Inner {
}

imply Inner {
    micro ping(self): unit {
    }
}

"#;

fn with_prelude(body: &str) -> String {
    format!("{PRELUDE}{body}")
}

fn compile(source: &str) -> Result<HirModule, HirError> {
    ValkyrieCompiler::new(SourceID::default()).compile_fixture_source(source)
}

fn compile_ok(body: &str) -> HirModule {
    compile(&with_prelude(body)).unwrap_or_else(|error| panic!("源码必须通过 HIR 阶段: {error}"))
}

fn find_struct<'a>(hir: &'a HirModule, name: &str) -> &'a HirStruct {
    hir.structs
        .iter()
        .find(|item| item.name.as_str() == name)
        .or_else(|| hir.submodules.iter().flat_map(|module| &module.structs).find(|item| item.name.as_str() == name))
        .unwrap_or_else(|| panic!("缺少结构 `{name}`"))
}

// ---------- 字段访问作为 receiver：既有推断覆盖 ----------

#[test]
fn struct_field_receiver_resolves() {
    compile_ok(
        r#"
class Outer {
    inner: Inner
}

[export]
micro probe(outer: Outer): unit {
    outer.inner.ping()
}
"#,
    );
}

#[test]
fn nested_struct_field_receiver_resolves() {
    compile_ok(
        r#"
class Nested {
    inner: Inner
}

class Outer {
    nested: Nested
}

[export]
micro probe(outer: Outer): unit {
    outer.nested.inner.ping()
}
"#,
    );
}

#[test]
fn labeled_tuple_field_receiver_resolves() {
    compile_ok(
        r#"
[export]
micro probe(pair: (inner: Inner, tag: bool)): unit {
    pair.inner.ping()
}
"#,
    );
}

#[test]
fn labeled_tuple_inside_struct_receiver_resolves() {
    compile_ok(
        r#"
class Outer {
    pair: (inner: Inner, tag: bool)
}

[export]
micro probe(outer: Outer): unit {
    outer.pair.inner.ping()
}
"#,
    );
}

#[test]
fn generic_struct_field_receiver_resolves() {
    compile_ok(
        r#"
class Outer<T> {
    inner: T
}

[export]
micro probe(outer: Outer<Inner>): unit {
    outer.inner.ping()
}
"#,
    );
}

#[test]
fn call_result_field_receiver_resolves() {
    compile_ok(
        r#"
class Outer {
    inner: Inner
}

micro make(): Outer {
    return Outer { inner: Inner {} }
}

[export]
micro probe(): unit {
    make().inner.ping()
}
"#,
    );
}

#[test]
fn self_field_receiver_resolves() {
    compile_ok(
        r#"
class Outer {
    inner: Inner
}

imply Outer {
    micro probe(self): unit {
        self.inner.ping()
    }
}
"#,
    );
}

#[test]
fn trait_typed_field_receiver_resolves() {
    compile_ok(
        r#"
trait Pipe {
}

imply Pipe {
    micro ping(self): unit {
    }
}

class Outer {
    pipe: Pipe
}

[export]
micro probe(outer: Outer): unit {
    outer.pipe.ping()
}
"#,
    );
}

#[test]
fn unlabeled_tuple_position_receiver_resolves() {
    compile_ok(
        r#"
[export]
micro probe(pair: (Inner, bool)): unit {
    pair.0.ping()
}
"#,
    );
}

#[test]
fn for_loop_over_field_access_resolves() {
    compile_ok(
        r#"
class Bag {
    items: [i32]
}

[export]
micro probe(bag: Bag): unit {
    for item in bag.items {
        let x: i32 = item
    }
}
"#,
    );
}

#[test]
fn cast_field_receiver_resolves() {
    compile_ok(
        r#"
class Outer {
    inner: Inner
}

[export]
micro probe(outer: Outer): unit {
    (outer as Outer).inner.ping()
}
"#,
    );
}

// ---------- 继承槽位合成 ----------

#[test]
fn default_parent_slot_is_a_registered_field() {
    let hir = compile_ok(
        r#"
class Base {
    inner: Inner
}

class Derived: Base {
}

[export]
micro probe(derived: Derived): unit {
    derived.base.inner.ping()
}
"#,
    );
    let base = find_struct(&hir, "Base");
    let derived = find_struct(&hir, "Derived");
    let base_declaration = base.constructor_declaration.expect("Base 须冻结构造器声明");
    assert_eq!(derived.fields.len(), 1, "空子类的字段空间只含合成的父类槽位: {:?}", derived.fields);
    let slot = &derived.fields[0];
    assert_eq!(slot.name.as_str(), "base", "无别名父类槽位取父类名的 snake_case");
    assert!(slot.id.is_some(), "父类槽位必须与声明体字段同路径分配 FieldId");
    assert!(slot.visibility.is_public(), "父类槽位按字段默认可见性暴露为 `obj.base`");
    assert_eq!(
        slot.ty,
        ValkyrieType::Nominal { declaration: base_declaration, arguments: Vec::new(), primitive_identity: None },
        "父类槽位类型必须冻结为父类声明身份"
    );
}

#[test]
fn parent_slots_follow_inheritance_chain_before_declared_fields() {
    let hir = compile_ok(
        r#"
class Base {
    inner: Inner
}

class Mid: Base {
}

class Derived: Mid {
    own: i32
}

[export]
micro probe(derived: Derived): unit {
    derived.mid.base.inner.ping()
}
"#,
    );
    let mid = find_struct(&hir, "Mid");
    let derived = find_struct(&hir, "Derived");
    let mid_declaration = mid.constructor_declaration.expect("Mid 须冻结构造器声明");
    let names = derived.fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>();
    assert_eq!(names, ["mid", "own"], "父类槽位在前、自身声明顺序在后");
    assert_eq!(
        derived.fields[0].ty,
        ValkyrieType::Nominal { declaration: mid_declaration, arguments: Vec::new(), primitive_identity: None },
        "父类槽位类型必须冻结为父类声明身份"
    );
    assert!(derived.fields.iter().all(|field| field.id.is_some()), "全部字段必须携带 FieldId");
    let mid_names = mid.fields.iter().map(|field| field.name.as_str()).collect::<Vec<_>>();
    assert_eq!(mid_names, ["base"], "继承链自上而下逐层合成槽位");
}

// ---------- 冲突负例 ----------

fn conflict_source(body: &str) -> String {
    let source = with_prelude(body);
    let error = compile(&source).expect_err("槽位冲突必须结构化失败");
    match &error {
        HirError::ParentSlotConflict { .. } => error.to_string(),
        other => panic!("必须产出 `ParentSlotConflict`，实际为: {other:?}"),
    }
}

#[test]
fn declared_field_shadowing_parent_slot_fails() {
    let error = conflict_source(
        r#"class Base {
    inner: Inner
}

class Derived: Base {
    base: Base
}
"#,
    );
    assert!(error.contains("field `base` on `Derived`"), "诊断必须点名冲突字段与聚合: {error}");
    assert!(error.contains("rename the field or alias the parent"), "诊断必须给出处置指引: {error}");
}

#[test]
fn duplicate_parent_slots_fail() {
    let error = conflict_source(
        r#"class Base {
    inner: Inner
}

class Twice: Base + Base {
}
"#,
    );
    assert!(error.contains("duplicate inherited slot `base` on `Twice`"), "诊断必须点名重复槽位与聚合: {error}");
}
