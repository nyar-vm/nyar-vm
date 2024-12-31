//! Valkyrie `.v` / `.vx` 源码格式化：扩展名事实归前端，formatter 经平台注册表解析。

use nyar_format::formatter::{FormatOptions, format_source_for_extension};
use rusty_valkyrie::source_surface::{SourceKind, VX_EXTENSIONS, V_EXTENSIONS};

fn assert_idempotent(extension: &str, source: &str) {
    rusty_valkyrie::register_format_providers();
    let options = FormatOptions::default();
    let once = format_source_for_extension(extension, source, &options).expect("format once");
    let twice = format_source_for_extension(extension, &once, &options).expect("format twice");
    assert_eq!(once, twice, "format should be idempotent");
}

#[test]
fn source_surface_owns_valkyrie_extension_facts() {
    assert_eq!(SourceKind::from_extension("v"), Some(SourceKind::V));
    assert_eq!(SourceKind::from_extension("valkyrie"), Some(SourceKind::V));
    assert_eq!(SourceKind::from_extension("vx"), Some(SourceKind::Vx));
    assert_eq!(SourceKind::from_extension("von"), Some(SourceKind::Von));
    assert_eq!(SourceKind::from_extension("awsl"), Some(SourceKind::Awsl));
    assert_eq!(SourceKind::from_extension("vk"), None);
}

#[test]
fn format_v_idempotent() {
    for &extension in V_EXTENSIONS {
        assert_idempotent(extension, "micro main(){let x=1}");
    }
}

#[test]
fn format_v_idempotent_with_comment() {
    for &extension in V_EXTENSIONS {
        assert_idempotent(extension, "# note\nmicro main(){let x=1}\n");
    }
}

#[test]
fn format_vx_idempotent() {
    for &extension in VX_EXTENSIONS {
        assert_idempotent(extension, "namespace demo;");
    }
}
