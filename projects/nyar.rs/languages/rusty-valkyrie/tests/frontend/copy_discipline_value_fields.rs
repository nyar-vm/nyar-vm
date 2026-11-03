//! 值类型（`structure`）含引用类型字段的 copy discipline 回归测试。
//!
//! 语言语义：值类型可含任意字段类型——class 实例、堆数组 `[T]`、`unite`
//! 均合法；值类型的拷贝为浅拷贝，引用字段拷贝引用本身。此前
//! `is_copy_compatible` 将这些字段一律判为 `copy discipline violation`
//! （`ReferenceTypedField`）属过严误判，本组测试锁定放宽后的行为。

use rusty_valkyrie::{SourceID, ValkyrieCompiler};

/// 数值原始类型不是语言内建，必须绑定到源码声明身份。
const PRIMITIVES: &str = "[primitive(\"std::primitive::i32\")] structure i32 { } ";

#[test]
fn structure_with_class_instance_field_is_accepted() {
    let compiler = ValkyrieCompiler::new(SourceID::default());
    compiler.compile_source("class Payload { } structure Holder { payload: Payload }").expect("class instance field must be accepted");
}

#[test]
fn structure_with_heap_array_field_is_accepted() {
    let compiler = ValkyrieCompiler::new(SourceID::default());
    compiler.compile_source(&format!("{PRIMITIVES}structure Holder {{ items: [i32] }}")).expect("heap array field must be accepted");
}

#[test]
fn structure_with_unite_field_is_accepted() {
    let compiler = ValkyrieCompiler::new(SourceID::default());
    compiler
        .compile_source(&format!("{PRIMITIVES}unite Choice {{ A {{ x: i32 }} B }} structure Holder {{ choice: Choice }}"))
        .expect("unite field must be accepted");
}

#[test]
fn reference_fields_compile_through_parameter_and_return_paths() {
    let compiler = ValkyrieCompiler::new(SourceID::default());
    compiler
        .compile_source(&format!(
            "{PRIMITIVES}\n{}",
            r#"
class Payload {
}

unite Choice {
    A { x: i32 }
    B
}

structure Holder {
    items: [i32]
    payload: Payload
    choice: Choice
}

micro pass(holder: Holder) -> Holder {
    return holder
}

[main]
micro entry() -> i32 {
    let holder: Holder = Holder { items: [], payload: Payload {}, choice: B }
    let moved: Holder = pass(holder)
    return 0
}
"#,
        ))
        .expect("reference-typed fields must compile through parameter and return paths");
}
