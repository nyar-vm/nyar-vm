use nyar_analyzer::{
    format::{FormatError, FormatOptions, FormattedOutput, SourceFormatter, SourceFormatterRegistry},
    *,
};

struct EchoFormatter;

impl SourceFormatter for EchoFormatter {
    fn language_id(&self) -> &str {
        "echo"
    }

    fn format_document(&self, source: &str, _options: &FormatOptions) -> Result<FormattedOutput, FormatError> {
        Ok(FormattedOutput::text_only(source.trim().to_string()))
    }
}

#[test]
fn registry_formats_by_alias() {
    let mut reg = SourceFormatterRegistry::new();
    reg.register(&["echo", "ech"], &["echo"], Box::new(EchoFormatter));
    assert!(reg.format("missing", "x", &FormatOptions::default()).is_err());
    let out = reg.format("ECHO", "  hi  ", &FormatOptions { ensure_trailing_newline: false, ..Default::default() }).unwrap();
    assert_eq!(out, "hi");
    assert_eq!(reg.extension_language_id("ECHO"), Some("echo"));
    assert!(reg.supports_extension("echo"));
    assert!(!reg.supports_extension("unknown"));
}
