//! Format parity integration tests (`.editorconfig`, language-neutral mappings).

use std::{fs, path::PathBuf};

use nyar_analyzer::format::FormatConfigLoader;
use nyar_format::formatter::{SourceKind, format_source};

fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("nyar_fmt_parity_{name}_{}", std::process::id()))
}

#[test]
fn editorconfig_maps_to_format_options() {
    let dir = temp_dir("ec_map");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".editorconfig"),
        r#"
root = true
[*.v]
indent_style = tab
indent_size = 2
tab_width = 4
max_line_length = 120
insert_final_newline = false
"#,
    )
    .unwrap();
    let file = dir.join("sample.v");
    fs::write(&file, "micro main(){}").unwrap();

    let res = FormatConfigLoader::for_path(&file);
    assert_eq!(res.options.indent_width, 2);
    assert_eq!(res.options.tab_size, 4);
    assert!(!res.options.insert_spaces);
    assert_eq!(res.options.max_width, 120);
    assert!(!res.options.ensure_trailing_newline);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn editorconfig_indent_applied_to_von_output() {
    let dir = temp_dir("ec_von");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".editorconfig"), "[*.von]\nindent_size = 2\n").unwrap();
    let file = dir.join("data.von");
    fs::write(&file, r#"{ "a": 1 }"#).unwrap();

    let opts = FormatConfigLoader::for_path(&file).options;
    assert_eq!(opts.indent_width, 2);
    let formatted = format_source(SourceKind::Von, r#"{ "a": 1 }"#, &opts).expect("von fmt");
    let again = format_source(SourceKind::Von, &formatted, &opts).expect("von idempotent");
    assert_eq!(formatted, again);

    let _ = fs::remove_dir_all(&dir);
}
