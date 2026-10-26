use std::fmt::Write as _;

use crate::asgard::awsl::{RenderIr, RenderNode, RenderRegionId, ScriptBinding, is_fragment_root};

use super::{
    aux_exports::AuxExports,
    bindings::emit_memoized_bindings,
    codegen::VRenderCodegen,
    routes::{hydrate_name_for_route, mount_name_for_route},
};

/// 将组件 RenderIR 生成为完整 V 模块（expr/evt 导出 + `awsl_hydrate_{route}` 或 `awsl_mount_{route}`）。
pub fn render_component_v(route: &str, bindings: &[ScriptBinding], script_prelude: &str, module: &RenderIr) -> String {
    if is_fragment_root(module) {
        return render_fragment_component_v(route, bindings, script_prelude, module);
    }
    let hydrate_name = hydrate_name_for_route(route);
    let mut aux = AuxExports::new(route);
    let mut hydrate_body = String::new();
    writeln!(hydrate_body, "    store_subscribe(\"{hydrate_name}_store\")").unwrap();
    if !script_prelude.trim().is_empty() {
        for line in script_prelude.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            writeln!(hydrate_body, "    {line}").unwrap();
        }
    }
    emit_memoized_bindings(bindings, &mut hydrate_body, route, &mut aux);
    let mut codegen = VRenderCodegen { route, module, bindings, aux: &mut aux, out: &mut hydrate_body, var_counter: 0, loop_stack: Vec::new() };
    let root = codegen.emit_roots();
    let mut out = String::new();
    aux.flush(&mut out);
    writeln!(out, "micro {hydrate_name}(): i32 {{").unwrap();
    out.push_str(&hydrate_body);
    writeln!(out, "    return {root}").unwrap();
    writeln!(out, "}}").unwrap();
    out
}

fn render_fragment_component_v(route: &str, bindings: &[ScriptBinding], script_prelude: &str, module: &RenderIr) -> String {
    let mount_name = mount_name_for_route(route);
    let mut aux = AuxExports::new(route);
    let mut mount_body = String::new();
    writeln!(mount_body, "    store_subscribe(\"{mount_name}_store\")").unwrap();
    if !script_prelude.trim().is_empty() {
        for line in script_prelude.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            writeln!(mount_body, "    {line}").unwrap();
        }
    }
    emit_memoized_bindings(bindings, &mut mount_body, route, &mut aux);
    let fragment_region = match module.roots.first().map(|&id| module.node(id)) {
        Some(RenderNode::Fragment(fragment)) => fragment.children,
        _ => RenderRegionId::EMPTY,
    };
    let mut codegen = VRenderCodegen { route, module, bindings, aux: &mut aux, out: &mut mount_body, var_counter: 0, loop_stack: Vec::new() };
    if fragment_region != RenderRegionId::EMPTY {
        codegen.emit_region_into(fragment_region, "container");
    }
    else {
        codegen.emit_roots_into("container");
    }
    let mut out = String::new();
    aux.flush(&mut out);
    writeln!(out, "micro {mount_name}(container: i32): i32 {{").unwrap();
    out.push_str(&mount_body);
    writeln!(out, "    return container").unwrap();
    writeln!(out, "}}").unwrap();
    out
}
