//! RenderIR → V hydrate 函数（Solid 细粒度绑定 + `dom.*` 宿主 API）。

mod aux_exports;
mod bindings;
mod codegen;
mod entry;
mod events;
pub(crate) mod expr;
mod loop_frame;
mod reactive;
mod routes;
pub(crate) mod util;

pub use entry::render_component_v;
pub use routes::{export_mount_name_for_route, export_name_for_route, hydrate_name_for_route, mount_name_for_route};
