use nyar_language::{
    types::{
        Identifier,
        hir::{HirDocumentation, HirStruct},
    },
    *,
};

#[test]
fn parse_doc_config_section() {
    let strukt = HirStruct {
        name: Identifier::new("AtlasAppConfig"),
        doc: HirDocumentation::from_lines(vec!["@config_section(\"atlas\")".to_string()]),
        fields: vec![],
        ..Default::default()
    };
    let section = struct_attr_string(&strukt, CONFIG_SECTION_ATTR);
    assert_eq!(section.as_deref(), Some("atlas"));
}

#[test]
fn manifest_includes_ws_and_health() {
    let routes = vec![AtlasRouteEntry {
        controller: "HealthController".into(),
        method_name: "get_live".into(),
        http_method: "GET".into(),
        path: "/health/live".into(),
        handler_id: "health_live".into(),
    }];
    let ws = vec![AtlasWsEntry {
        path: "/ws/echo".into(),
        handler_id: "echo".into(),
        controller: "EchoWsHandler".into(),
        method_name: "on_event".into(),
    }];
    let health = vec![AtlasHealthCheckEntry { name: "db".into(), controller: "HealthController".into(), method_name: "check_db".into() }];
    let manifest = merge_manifest(&routes, &ws, &health, &[]);
    assert!(manifest.contains("ws_routes"));
    assert!(manifest.contains("health_checks"));
    assert!(manifest.contains("/health/live"));
}
