//! Migrated from `src/dev_server.rs` (`tests`).

use rusty_valkyrie::asgard::dev_server::*;

#[test]
fn hmr_client_uses_sse_and_hot_swap() {
    let script = generate_hmr_client_script();
    assert!(script.contains("/__asgard/hmr/events"));
    assert!(script.contains("WebAssembly.instantiate"));
    assert!(!script.contains("setInterval(poll"));
}
