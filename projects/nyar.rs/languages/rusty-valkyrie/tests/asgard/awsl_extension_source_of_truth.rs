//! AWSL 后缀单一事实源收敛测试：各识别站点必须与 `nyar_format::formatter::AWSL_EXTENSIONS` 同源。

use rusty_valkyrie::source_surface::{AWSL_EXTENSIONS, source_kind_from_extension};

#[test]
fn source_surface_aliases_the_platform_extension_set() {
    // 第二份集合必须与单一事实源逐元素一致，防止再次漂移。
    assert_eq!(AWSL_EXTENSIONS, nyar_format::formatter::AWSL_EXTENSIONS);
    assert_eq!(AWSL_EXTENSIONS, &["awsl", "w"]);
}

#[test]
fn source_kind_maps_both_suffixes_to_awsl() {
    // LSP formatting 经 `source_kind_from_extension` 判定；`.w` 必须与 `.awsl` 同身份。
    use rusty_valkyrie::source_surface::SourceKind;
    assert_eq!(source_kind_from_extension("awsl"), Some(SourceKind::Awsl));
    assert_eq!(source_kind_from_extension("w"), Some(SourceKind::Awsl));
    assert_eq!(source_kind_from_extension("aws"), None);
    assert_eq!(source_kind_from_extension("v"), Some(SourceKind::V));
    assert_eq!(source_kind_from_extension("vx"), Some(SourceKind::Vx));
}

#[test]
fn is_awsl_extension_predicate() {
    assert!(nyar_format::formatter::is_awsl_extension("awsl"));
    assert!(nyar_format::formatter::is_awsl_extension("w"));
    assert!(!nyar_format::formatter::is_awsl_extension("aws"));
    assert!(!nyar_format::formatter::is_awsl_extension("awslw"));
    assert!(!nyar_format::formatter::is_awsl_extension(""));
}
