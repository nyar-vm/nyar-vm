//! Valkyrie `.v` / `.vx` 源码格式化（先注册前端 provider，再走平台注册表）。

use nyar_format::formatter::{FormatOptions, SourceKind, format_source};

fn assert_idempotent(kind: SourceKind, source: &str) {
    rusty_valkyrie::register_format_providers();
    let options = FormatOptions::default();
    let once = format_source(kind, source, &options).expect("format once");
    let twice = format_source(kind, &once, &options).expect("format twice");
    assert_eq!(once, twice, "format should be idempotent");
}

#[test]
fn format_v_idempotent() {
    assert_idempotent(SourceKind::V, "micro main(){let x=1}");
}

#[test]
fn format_v_idempotent_with_comment() {
    assert_idempotent(SourceKind::V, "# note\nmicro main(){let x=1}\n");
}

#[test]
fn format_vx_idempotent() {
    assert_idempotent(SourceKind::Vx, "namespace demo;");
}
