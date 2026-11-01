//! AWSL 扩展名单一事实源：`.awsl` 规范拼写与同义缩写 `.w` 同 parser、同 lowering、同身份。

use nyar_format::formatter::{AWSL_EXTENSIONS, FormatBatchOptions, FormatCliOptions, FormatOptions, SourceKind, format_paths, format_source_for_extension, is_awsl_extension};

const SAMPLE: &str = r#"
<template>
<div class="box"><span>{title}</span></div>
</template>
"#;

#[test]
fn extension_set_is_awsl_and_w() {
    assert_eq!(AWSL_EXTENSIONS, &["awsl", "w"]);
    assert!(is_awsl_extension("awsl"));
    assert!(is_awsl_extension("w"));
    assert!(!is_awsl_extension("aws"));
    assert!(!is_awsl_extension("vx"));
}

#[test]
fn w_and_awsl_format_to_identical_output() {
    let options = FormatOptions::default();
    let awsl = format_source_for_extension("awsl", SAMPLE, &options).expect(".awsl 应可格式化");
    let w = format_source_for_extension("w", SAMPLE, &options).expect(".w 应可格式化");
    assert_eq!(awsl, w, ".w 与 .awsl 应走同一 formatter、产出相同");
    let twice = format_source_for_extension("w", &awsl, &options).expect(".w 幂等重排应成功");
    assert_eq!(awsl, twice, "格式化应幂等");
}

#[test]
fn source_kind_recognizes_both_suffixes() {
    assert_eq!(SourceKind::from_extension("awsl"), Some(SourceKind::Awsl));
    assert_eq!(SourceKind::from_extension("w"), Some(SourceKind::Awsl));
    assert_eq!(SourceKind::from_extension("aws"), None);
    assert_eq!(SourceKind::from_path(std::path::Path::new("source/index.w")), Some(SourceKind::Awsl));
}

#[test]
fn awsl_only_cli_filter_covers_both_suffixes() {
    let options = FormatCliOptions::default().awsl_only();
    assert_eq!(options.extensions, Some(vec!["awsl".to_string(), "w".to_string()]));
}

#[test]
fn batch_discovers_w_file_and_writes_it() {
    let dir = std::env::temp_dir().join(format!("nyar_fmt_awsl_w_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.w"), SAMPLE).unwrap();

    let check = FormatBatchOptions { check: true, ..FormatBatchOptions::default() };
    let report = format_paths(&[dir.clone()], &check).expect("check .w");
    assert_eq!(report.checked, 1, "未指定扩展名过滤时 .w 应被注册表发现");

    let awsl_only = FormatBatchOptions { check: true, extensions: Some(vec!["awsl".into(), "w".into()]), ..FormatBatchOptions::default() };
    let report = format_paths(&[dir.clone()], &awsl_only).expect("awsl_only 过滤应覆盖 .w");
    assert_eq!(report.checked, 1);

    let _ = std::fs::remove_dir_all(&dir);
}
