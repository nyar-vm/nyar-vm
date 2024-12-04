//! Valkyrie 格式化 parity（`.editorconfig` 无关的 V 断言）。

use nyar_format::formatter::{FormatOptions, SourceKind, format_source};

fn format_v_twice(source: &str) -> String {
    rusty_valkyrie::register_format_providers();
    let options = FormatOptions::default();
    let once = format_source(SourceKind::V, source, &options).expect("format once");
    let twice = format_source(SourceKind::V, &once, &options).expect("format twice");
    assert_eq!(once, twice);
    once
}

#[test]
fn format_idempotent_for_v_with_comment() {
    let once = format_v_twice("# header\nmicro main(){let x=1}\n");
    assert!(once.contains('#'), "comment should be preserved");
}

#[test]
fn golden_v_micro_main_idempotent() {
    let out = format_v_twice("micro main(){let x=1}");
    assert_eq!(format_source(SourceKind::V, &out, &FormatOptions::default()).unwrap(), out);
}

#[test]
fn flags_members_format_idempotent() {
    let out = format_v_twice("flags FilePerm { Read = 1 Write = 2 }");
    assert!(out.contains("Read"));
    assert!(out.contains("Write"));
}

#[test]
fn enums_variants_format_idempotent() {
    let out = format_v_twice("enums Color { RED=2 GREEN=4 BLUE=6 }");
    assert!(out.contains("RED"));
    assert!(out.contains("GREEN"));
    assert!(out.contains("BLUE"));
}
