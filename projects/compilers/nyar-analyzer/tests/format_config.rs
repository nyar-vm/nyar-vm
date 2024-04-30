use nyar_analyzer::{format::FormatConfigLoader, *};
use std::fs;

#[test]
fn editorconfig_maps_indent_and_line_length() {
    let dir = std::env::temp_dir().join(format!("nyar_fmt_cfg_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(".editorconfig"),
        r#"
root = true
[*.v]
indent_style = space
indent_size = 2
max_line_length = 100
insert_final_newline = false
"#,
    )
    .unwrap();
    let file = dir.join("sample.v");
    fs::write(&file, "micro main(){}").unwrap();

    let res = FormatConfigLoader::for_path(&file);
    assert_eq!(res.options.indent_width, 2);
    assert_eq!(res.options.max_width, 100);
    assert!(!res.options.ensure_trailing_newline);
    assert!(res.editorconfig_keys.contains("indent_size"));
    assert!(res.editorconfig_keys.contains("max_line_length"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn lsp_does_not_override_editorconfig_indent() {
    let dir = std::env::temp_dir().join(format!("nyar_fmt_cfg_lsp_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".editorconfig"), "[*.v]\nindent_size = 4\nmax_line_length = 80\n").unwrap();
    let file = dir.join("a.v");
    fs::write(&file, "").unwrap();
    let res = FormatConfigLoader::for_path(&file);
    let merged = FormatConfigLoader::merge_lsp(res, 8, false, Some(true));
    assert_eq!(merged.indent_width, 4);
    assert_eq!(merged.tab_size, 4);

    let _ = fs::remove_dir_all(&dir);
}
