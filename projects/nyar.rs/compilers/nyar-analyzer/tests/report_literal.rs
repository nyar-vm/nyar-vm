use nyar_analyzer::report::{ColSeriesItem, series_to_init_literal};

#[test]
fn empty_series_is_empty_list() {
    assert_eq!(series_to_init_literal(&[]), "[]");
}

#[test]
fn serializes_struct_fields() {
    let lit = series_to_init_literal(&[ColSeriesItem {
        key: "pass".into(),
        label: "pass".into(),
        value_text: "2.0 tests".into(),
        fill: "#22c55e".into(),
        height_pct: 100.0,
    }]);
    assert!(lit.starts_with("[asgard.plotter::ColSeriesItem {"), "元素必须带构造式类型:\n{lit}");
    assert!(lit.contains("key: \"pass\""));
    assert!(lit.contains("height_pct: 100.0"));
}

/// VX 词法没有反斜杠转义：标签里的引号必须靠更大的引号 run 容纳，
/// 花括号必须写成 `\{`，否则注入脚本解析失败、ABI 提取为空。
#[test]
fn quotes_and_braces_are_encoded_for_vx_lexer() {
    let lit = series_to_init_literal(&[ColSeriesItem {
        key: "a\"b".into(),
        label: "show {count}".into(),
        value_text: "2.0".into(),
        fill: "#fff".into(),
        height_pct: 50.0,
    }]);
    assert!(lit.contains("\"\"\"a\"b\"\"\""), "含引号的值需要 3 引号定界:\n{lit}");
    // 只有 `{` 开启插值，必须转义；`}` 在插值外是普通文本，无需转义。
    assert!(lit.contains("show \\{count}"), "开插值花括号必须转义:\n{lit}");
    assert!(!lit.contains("\\\""), "VX 没有反斜杠转义，不得写出 \\\" :\n{lit}");
}
