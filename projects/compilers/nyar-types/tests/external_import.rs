use nyar_types::{ExternalImportLink, Identifier};

#[test]
fn builds_host_link_for_clr_console_write_line() {
    let link = ExternalImportLink::host(
        Some(Identifier::new("clr")),
        vec!["mscorlib".to_string(), "System.Console".to_string(), "WriteLine".to_string()],
    );

    assert!(link.matches_boundary("host"));
    assert!(link.matches_platform_tag("clr"));
    assert!(link.matches_host_platform("clr"));
    assert_eq!(link.locator_segments(), &["mscorlib".to_string(), "System.Console".to_string(), "WriteLine".to_string(),]);
}

#[test]
fn renders_stable_debuggable_display_shape() {
    let link =
        ExternalImportLink::host(Some(Identifier::new("wasi")), vec!["wasi:io/streams".to_string(), "blocking-write-and-flush".to_string()]);

    assert_eq!(link.to_string(), "host[wasi](\"wasi:io/streams\", \"blocking-write-and-flush\")");
}
