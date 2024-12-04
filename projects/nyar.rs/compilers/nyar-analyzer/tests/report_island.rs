use nyar_analyzer::report::{IslandKind, awsl_island_kind};

#[test]
fn chart_routes_are_hydrated() {
    assert_eq!(awsl_island_kind("charts/chart-status.awsl"), IslandKind::Hydrated);
}

#[test]
fn report_pages_are_static() {
    assert_eq!(awsl_island_kind("pages/test-report.awsl"), IslandKind::Static);
    assert_eq!(awsl_island_kind("pages/layout.awsl"), IslandKind::Static);
}

#[test]
fn server_island_paths_are_server() {
    assert_eq!(awsl_island_kind("islands/server/user-card.awsl"), IslandKind::Server);
    assert_eq!(awsl_island_kind("source/islands/server/x.awsl"), IslandKind::Server);
}
