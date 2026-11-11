//! 文本常量与文本载体类型的同一身份回归。
//!
//! std 的 `type utf8 = Utf8Text` 是别名，`Utf8Text` 是文本载体声明；`utf8` 别名、
//! 显式 `Utf8Text`、跨包 `using` 三种写法必须落到同一冻结声明身份。字符串常量在
//! MIR 中保持编码载体 `Utf8`，不因聚合字段声明而被改写；其他 class 的常量不得被
//! 当成文本载体。

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    hir::lowering::DEFAULT_COMPILE_ARCH,
    types::hir::{HirModule, ValkyrieType},
};

fn core() -> CompilerSourceGroup {
    CompilerSourceGroup::single_source(
        "core",
        "std.text",
        r#"
namespace std.operator;

trait Equal {
    infix `==`(self, other: Self): bool

    infix `!=`(self, other: Self): bool
}

namespace std.text;

[primitive("std::text::utf8")]
class Utf8Text {
    _repr: [u8]
}

class Widget {
    value: i32
}

type utf8 = Utf8Text;

imply Utf8Text: std::operator::Equal {
    infix `==`(self, other: Self): bool { return true }

    infix `!=`(self, other: Self): bool { return false }
}
"#,
    )
}

fn app(source: &str) -> CompilerSourceGroup {
    CompilerSourceGroup::single_source("app", "demo", source).as_entry().with_direct_dependencies(vec!["core".into()])
}

fn resolve(groups: &[CompilerSourceGroup]) -> Vec<HirModule> {
    let groups = crate::primitives::groups_with_primitives(groups.to_vec());
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("文本载体身份必须完成 HIR 冻结")
}

fn compile(groups: &[CompilerSourceGroup]) -> Result<(), String> {
    let groups = crate::primitives::groups_with_primitives(groups.to_vec());
    ValkyrieCompiler::default().compile_source_groups_to_program(&groups).map(|_| ()).map_err(|error| format!("{error:?}"))
}

fn declaration(modules: &[HirModule], group_key: &str, name: &str) -> nyar_types::ItemId {
    modules
        .iter()
        .find(|module| module.dependency_key == group_key)
        .and_then(|module| module.structs.iter().find(|structure| structure.name.as_str() == name))
        .and_then(|structure| structure.constructor_declaration)
        .unwrap_or_else(|| panic!("missing declaration `{name}` in group `{group_key}`"))
}

fn field_type(modules: &[HirModule], group_key: &str, structure_name: &str, field_name: &str) -> ValkyrieType {
    modules
        .iter()
        .find(|module| module.dependency_key == group_key)
        .and_then(|module| module.structs.iter().find(|structure| structure.name.as_str() == structure_name))
        .and_then(|structure| structure.fields.iter().find(|field| field.name.as_str() == field_name))
        .map(|field| field.ty.clone())
        .unwrap_or_else(|| panic!("missing field `{structure_name}.{field_name}` in group `{group_key}`"))
}

#[test]
fn local_alias_field_keeps_carrier_declaration_identity() {
    let groups = [CompilerSourceGroup::single_source(
        "app",
        "demo",
        r#"
namespace std.text;

[primitive("std::text::utf8")]
class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text;

structure Label {
    text: utf8
}

[export]
micro probe() -> Label {
    return Label { text: "hello" }
}
"#,
    )
    .as_entry()];
    let modules = resolve(&groups);
    assert_eq!(
        field_type(&modules, "app", "Label", "text"),
        ValkyrieType::Nominal { declaration: declaration(&modules, "app", "Utf8Text"), arguments: Vec::new(), primitive_identity: None }
    );
    compile(&groups).expect("`utf8` 别名常量必须与文本载体同一身份");
}

#[test]
fn explicit_nominal_field_keeps_carrier_declaration_identity() {
    let groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{Utf8Text}

structure Label {
    text: Utf8Text
}

[export]
micro probe() -> Label {
    return Label { text: "hello" }
}
"#),
    ];
    let modules = resolve(&groups);
    assert_eq!(
        field_type(&modules, "app", "Label", "text"),
        ValkyrieType::Nominal { declaration: declaration(&modules, "core", "Utf8Text"), arguments: Vec::new(), primitive_identity: None }
    );
    compile(&groups).expect("显式 `Utf8Text` 常量必须与文本载体同一身份");
}

#[test]
fn cross_package_alias_using_keeps_carrier_declaration_identity() {
    let groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{utf8}

structure Label {
    text: utf8
}

[export]
micro probe() -> Label {
    return Label { text: "hello" }
}
"#),
    ];
    let modules = resolve(&groups);
    // 三种写法落到同一冻结声明身份：跨包别名展开后仍是 `Utf8Text` 声明。
    assert_eq!(
        field_type(&modules, "app", "Label", "text"),
        ValkyrieType::Nominal { declaration: declaration(&modules, "core", "Utf8Text"), arguments: Vec::new(), primitive_identity: None }
    );
    compile(&groups).expect("跨包 `using` 的 `utf8` 常量必须与文本载体同一身份");
}

#[test]
fn text_constant_in_nominal_comparison_keeps_encoding_identity() {
    let groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{utf8}

micro same(value: utf8) -> bool {
    return value == "hello"
}

[export]
micro probe() -> bool {
    let label: utf8 = "hello"
    return same(label)
}
"#),
    ];
    compile(&groups).expect("文本常量比较不得被改写成第二身份");
}

#[test]
fn value_if_merges_literal_and_nominal_text_carrier() {
    // 值位置 `if` 的一臂是字符串字面量（编码载体 `Utf8`），另一臂是返回 `utf8` 的调用
    // （名义声明 `Utf8Text`）。二者同处文本值空间，SSA 合并块参数与两条入边必须按
    // 01 §5.4 的同一冻结编码身份判等，不得在 MIR 合同或 canonical block-parameter 处
    // 报 SMIR007 / CAN015。
    let groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{utf8}

micro wrap(value: utf8) -> utf8 {
    return value
}

[export]
micro probe(flag: bool) -> utf8 {
    let base: utf8 = "x"
    let merged: utf8 = if flag { "" } else { wrap(base) }
    return merged
}
"#),
    ];
    compile(&groups).expect("文本字面量与名义文本载体的值合并必须同型");
}

#[test]
fn value_if_rejects_text_and_integer_merge() {
    // 反向：文本臂与整数臂不属同一值空间，合并必须失败关闭，证明放行只针对文本 carrier，
    // 没有把块参数/入边类型合同整体放宽。
    let groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{utf8}

[export]
micro probe(flag: bool) -> utf8 {
    let merged: utf8 = if flag { "" } else { 1 }
    return merged
}
"#),
    ];
    compile(&groups).expect_err("文本与整数不得合并为同一 SSA 值");
}

#[test]
fn other_class_constant_is_not_a_text_carrier() {
    let identity_groups = [
        core(),
        app(r#"
namespace demo;

using std.text::{utf8, Utf8Text, Widget}

structure Label {
    text: Widget
}

[export]
micro probe(label: Label) -> Widget {
    return label.text
}
"#),
    ];
    let modules = resolve(&identity_groups);
    let widget = declaration(&modules, "core", "Widget");
    let utf8_text = declaration(&modules, "core", "Utf8Text");
    assert_ne!(widget, utf8_text);
    assert_eq!(
        field_type(&modules, "app", "Label", "text"),
        ValkyrieType::Nominal { declaration: widget, arguments: Vec::new(), primitive_identity: None }
    );

    let group_with_value = app(r#"
namespace demo;

using std.text::{utf8, Utf8Text, Widget}

structure Label {
    text: Widget
}

[export]
micro probe() -> Label {
    return Label { text: "hello" }
}
"#);
    compile(&[core(), group_with_value]).expect_err("字符串常量不得写入非文本 class 字段");

    let group_with_return = app(r#"
namespace demo;

using std.text::{utf8, Utf8Text, Widget}

[export]
micro reject(value: Widget) -> utf8 {
    return value
}
"#);
    compile(&[core(), group_with_return]).expect_err("`Widget` 值不得被当成 `Utf8Text`");
}
