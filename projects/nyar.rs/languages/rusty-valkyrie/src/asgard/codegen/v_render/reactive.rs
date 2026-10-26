use crate::asgard::awsl::{
    RenderModule, RenderNode, RenderRegionId, RenderTextSegment, ScriptBinding,
    render_ir::{attr_value_source, region_nodes},
};

fn expr_uses_reactive(expr: &str, bindings: &[ScriptBinding]) -> bool {
    bindings.iter().any(|b| b.reactive && expr.contains(&b.name))
}

fn node_uses_reactive(module: &RenderModule, node_id: crate::asgard::awsl::RenderNodeId, bindings: &[ScriptBinding]) -> bool {
    match module.node(node_id) {
        RenderNode::Element(element) => {
            for attr in &element.attrs {
                let src = attr_value_source(module, &attr.value);
                if expr_uses_reactive(&src, bindings) {
                    return true;
                }
            }
            region_uses_reactive(module, element.children, bindings)
        }
        RenderNode::Component(component) => {
            for attr in &component.attrs {
                let src = attr_value_source(module, &attr.value);
                if expr_uses_reactive(&src, bindings) {
                    return true;
                }
            }
            region_uses_reactive(module, component.children, bindings)
        }
        RenderNode::Text(text) => text.segments.iter().any(|segment| match segment {
            RenderTextSegment::Expr(expr_id) => expr_uses_reactive(module.expr_source(*expr_id), bindings),
            RenderTextSegment::Static(_) => false,
        }),
        RenderNode::If(render_if) => {
            expr_uses_reactive(module.expr_source(render_if.condition), bindings)
                || region_uses_reactive(module, render_if.then_region, bindings)
                || region_uses_reactive(module, render_if.else_region, bindings)
        }
        RenderNode::Loop(render_loop) => {
            expr_uses_reactive(module.expr_source(render_loop.items), bindings)
                || region_uses_reactive(module, render_loop.body_region, bindings)
        }
        RenderNode::Fragment(fragment) => region_uses_reactive(module, fragment.children, bindings),
    }
}

pub(crate) fn region_uses_reactive(module: &RenderModule, region: RenderRegionId, bindings: &[ScriptBinding]) -> bool {
    for &node_id in region_nodes(module, region) {
        if node_uses_reactive(module, node_id, bindings) {
            return true;
        }
    }
    false
}
