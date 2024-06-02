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
    assert!(lit.contains("key: \"pass\""));
    assert!(lit.contains("height_pct: 100.0"));
}
