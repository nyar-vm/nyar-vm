use super::util::sanitize_export;

/// 路由的 hydrate 导出名（`awsl_hydrate_<route>`）。
pub fn hydrate_name_for_route(route_name: &str) -> String {
    format!("awsl_hydrate_{}", sanitize_export(route_name))
}

/// 路由的 mount 导出名（`awsl_mount_<route>`）。
pub fn mount_name_for_route(route_name: &str) -> String {
    format!("awsl_mount_{}", sanitize_export(route_name))
}

/// 路由的导出名（与 hydrate 名一致）。
pub fn export_name_for_route(route_name: &str) -> String {
    hydrate_name_for_route(route_name)
}

/// 路由的 mount 导出名。
pub fn export_mount_name_for_route(route_name: &str) -> String {
    mount_name_for_route(route_name)
}
