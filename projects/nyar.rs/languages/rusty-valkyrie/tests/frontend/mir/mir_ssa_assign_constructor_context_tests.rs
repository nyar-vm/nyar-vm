//! 赋值值位置以左值声明类型作为期望类型。
//!
//! 回归 `leetcode.v` 1546：`seen = HashMap::new(0)` 之后紧跟 `seen.insert(0, 1)`，
//! 赋值值位置曾沿用外层上下文（无期望类型），泛型 `new` 解析不出实例，
//! MIR 把调用降为 Unit，随后接收者类型不符报 SMIR007。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

fn groups(main_source: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace std.collection;

class Holder<K, V> {
    _capacity: i64
}

imply Holder<K, V> {
    micro new(capacity: i64): Self {
        return Holder { _capacity: capacity }
    }

    micro insert(self, key: i64, value: i64): i64 {
        self._capacity = key
        return value
    }
}
"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            &format!(
                r#"
namespace demo;

using std.collection::{{Holder, i64}}

{main_source}
"#
            ),
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

#[test]
fn assignment_value_uses_target_declared_type() {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups(
            r#"
[export]
micro probe(flag: bool) -> i64 {
    let mut h: Holder<i64, i64> = Holder::new(4)
    h.insert(1, 2)
    if flag {
        h = Holder::new(4)
        h.insert(1, 2)
    }
    return 0
}
"#,
        ))
        .unwrap_or_else(|error| panic!("赋值值位置必须用左值声明类型定下泛型实例，实际：{error}"));
    assert!(!program.canonical().mir.functions.is_empty());
}

#[test]
fn assignment_without_declared_target_type_still_errors() {
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups(
            r#"
[export]
micro probe(flag: bool) -> i64 {
    let mut h = Holder::new(4)
    h.insert(1, 2)
    if flag {
        h = Holder::new(4)
    }
    return 0
}
"#,
        ))
        .expect_err("没有声明类型时不得静默选 Holder<i64, i64>");
    assert!(
        !error.to_string().trim().is_empty(),
        "无法推断时必须给出结构化诊断"
    );
}
