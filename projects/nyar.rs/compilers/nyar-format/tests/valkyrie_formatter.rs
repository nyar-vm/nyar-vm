//! 平台拥有的 VON / AWSL 源码格式化（前端语言的扩展名由 provider 注册）。

use nyar_format::formatter::{FormatBatchOptions, FormatOptions, SourceKind, format_paths, format_source, format_source_for_extension};

fn assert_idempotent(kind: SourceKind, source: &str) {
    let options = FormatOptions::default();
    let once = format_source(kind, source, &options).expect("format once");
    let twice = format_source(kind, &once, &options).expect("format twice");
    assert_eq!(once, twice, "format should be idempotent");
}

#[test]
fn platform_source_kind_from_extension() {
    assert_eq!(SourceKind::from_extension("von"), Some(SourceKind::Von));
    assert_eq!(SourceKind::from_extension("awsl"), Some(SourceKind::Awsl));
    // 前端语言（如 `.v` / `.vx`）的扩展名不由平台枚举判定。
    assert_eq!(SourceKind::from_extension("v"), None);
    assert_eq!(SourceKind::from_extension("vx"), None);
    assert_eq!(SourceKind::from_extension("rs"), None);
}

#[test]
fn format_von_by_registered_extension() {
    let options = FormatOptions::default();
    let formatted = format_source_for_extension("von", r#"{ "a": 1 }"#, &options).expect("von fmt");
    assert_eq!(format_source_for_extension("von", &formatted, &options).expect("von idempotent"), formatted);
}

#[test]
fn format_awsl_idempotent() {
    let source = r#"
<template>
<div class="box"><span>{title}</span></div>
</template>
"#;
    assert_idempotent(SourceKind::Awsl, source);
}

#[test]
fn format_von_idempotent_with_comment() {
    assert_idempotent(SourceKind::Von, "# note\n{ x: 1 }\n");
}

#[test]
fn format_von_idempotent() {
    assert_idempotent(SourceKind::Von, r#"{ "x": 1, "y": [true, null] }"#);
}

#[test]
fn format_paths_check_detects_dirty() {
    let dir = std::env::temp_dir().join(format!("nyar_fmt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("sample.von");
    std::fs::write(&path, r#"{ "a":1 }"#).unwrap();

    let check = FormatBatchOptions { check: true, ..FormatBatchOptions::default() };
    let report = format_paths(&[dir.clone()], &check).expect("check");
    assert_eq!(report.checked, 1);
    assert_eq!(report.changed, 1);

    let write = FormatBatchOptions { check: false, ..FormatBatchOptions::default() };
    let report = format_paths(&[dir.clone()], &write).expect("write");
    assert_eq!(report.changed, 1);

    let report = format_paths(&[dir.clone()], &check).expect("recheck");
    assert_eq!(report.changed, 0);

    let _ = std::fs::remove_dir_all(&dir);
}
