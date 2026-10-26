//! Migrated from `src/host_backend.rs` (`tests`).

use rusty_valkyrie::asgard::host_backend::*;

#[test]
fn terminal_maps_to_terminal_backend() {
    assert_eq!(HostBackend::from_platform(HostPlatform::Terminal), Ok(HostBackend::Terminal));
}
