//! 纯类型级标记字段（零字段值类型 + 泛型形参）不进入 Semantic MIR 聚合字段事实与运行时布局。

use rusty_valkyrie::{ValkyrieCompiler, mir::ssa::MirLowerer};

const SOURCE: &str = r#"
namespace test.marker;

structure Phantom<T> {
}

structure Empty {
}

class Array<T> {
    _address: bool,
    _length: bool,
    _typing: Phantom<T>,
}

class Holder<T> {
    _flag: bool,
    _empty: Empty,
}
"#;

fn field_names(declaration_name: &str) -> Vec<String> {
    let hir = ValkyrieCompiler::default().compile_source(SOURCE).expect("源码必须通过 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);
    mir.structs
        .iter()
        .find(|declaration| declaration.name == declaration_name)
        .unwrap_or_else(|| panic!("缺少聚合 `{declaration_name}`"))
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect()
}

#[test]
fn drops_marker_field_from_aggregate_facts() {
    // `Phantom<T>` 零字段且只通过类型实参参与身份，其包装字段无运行时事实。
    assert_eq!(field_names("Array"), vec!["_address".to_string(), "_length".to_string()]);
}

#[test]
fn keeps_zero_field_non_marker_field() {
    // 无泛型的空结构不是纯类型级标记，字段必须保留。
    assert_eq!(field_names("Holder"), vec!["_flag".to_string(), "_empty".to_string()]);
    assert!(field_names("Empty").is_empty());
}
