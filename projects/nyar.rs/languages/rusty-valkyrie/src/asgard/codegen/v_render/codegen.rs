use std::fmt::Write as _;

use crate::asgard::awsl::{
    RenderAttr, RenderAttrValue, RenderIfNode, RenderLoopNode, RenderModule, RenderNode, RenderNodeId, RenderRegionId, RenderTextSegment,
    ScriptBinding,
    expr_deps::reactive_deps,
    render_ir::{attr_value_source, region_nodes},
};

use super::{
    aux_exports::AuxExports,
    events::{EventArgKind, EventHandler, parse_event_handler, split_field_access, strip_handler_wrapper},
    expr::{rewrite_expr_with_loops, rewrite_utf8_expr_body},
    loop_frame::LoopFrame,
    reactive::region_uses_reactive,
    routes::hydrate_name_for_route,
    util::{escape_v_string, map_awsl_component_to_host, sanitize_export},
};

pub(crate) struct VRenderCodegen<'a> {
    pub route: &'a str,
    pub module: &'a RenderModule,
    pub bindings: &'a [ScriptBinding],
    pub aux: &'a mut AuxExports,
    pub out: &'a mut String,
    pub var_counter: u32,
    pub loop_stack: Vec<LoopFrame>,
}

impl<'a> VRenderCodegen<'a> {
    fn next_var(&mut self) -> String {
        let name = format!("h{}", self.var_counter);
        self.var_counter += 1;
        name
    }

    fn next_export_id(&mut self, kind: &str) -> String {
        let id = self.aux.items.len();
        format!("awsl_{kind}_{}_{id}", sanitize_export(self.route))
    }

    fn dep_args(&self, expr: &str) -> (String, String) {
        let mut ids: Vec<String> = reactive_deps(expr, self.bindings)
            .iter()
            .filter_map(|name| self.bindings.iter().find(|b| b.name == *name))
            .map(|b| b.sig_id_expr())
            .collect();
        if (expr.contains("get_") || expr.contains("count_") || expr.contains("todos(")) && ids.len() < 2 {
            ids.push("-2".into());
        }
        let dep1 = ids.first().cloned().unwrap_or_else(|| "-1".into());
        let dep2 = ids.get(1).cloned().unwrap_or_else(|| "-1".into());
        (dep1, dep2)
    }

    fn rewrite_expr(&self, expr: &str) -> String {
        rewrite_expr_with_loops(expr, self.bindings, &self.loop_stack)
    }

    fn register_utf8_expr(&mut self, expr: &str) -> String {
        let name = self.next_export_id("expr");
        let body = rewrite_utf8_expr_body(expr, self.bindings, &self.loop_stack);
        self.aux.push(format!("micro {name}(): utf8 {{\n    return {body}\n}}"));
        name
    }

    fn register_bool_expr(&mut self, expr: &str) -> String {
        let name = self.next_export_id("cond");
        let rewritten = self.rewrite_expr(expr);
        self.aux.push(format!("micro {name}(): bool {{\n    return {rewritten}\n}}"));
        name
    }

    fn register_event(&mut self, handler: &str) -> (String, Option<EventArgKind>) {
        let name = self.next_export_id("evt");
        let handler = strip_handler_wrapper(handler);
        match parse_event_handler(&handler) {
            EventHandler::Simple(fn_name) => {
                self.aux.push(format!(
                    "micro {name}() {{\n    rx_batch_begin()\n    awsl_call_{}()\n    store_bump()\n    rx_batch_end()\n}}",
                    sanitize_export(&fn_name)
                ));
                (name, None)
            }
            EventHandler::Call { fn_name, arg } => {
                let (body, kind) = self.emit_call_event_body(&name, &fn_name, &arg);
                self.aux.push(body);
                (name, kind)
            }
            EventHandler::Unknown => {
                self.aux.push(format!("micro {name}() {{\n    rx_batch_begin()\n    store_bump()\n    rx_batch_end()\n}}"));
                (name, None)
            }
        }
    }

    fn emit_call_event_body(&self, evt_name: &str, fn_name: &str, arg: &str) -> (String, Option<EventArgKind>) {
        let fn_export = sanitize_export(fn_name);
        let arg = arg.trim();
        if let Some(frame) = self.loop_stack.iter().rev().find(|f| f.item_var == arg) {
            let items = &frame.items_expr;
            let body = format!(
                "micro {evt_name}(index: i32) {{\n    rx_batch_begin()\n    awsl_call_{fn_export}({items}[index])\n    store_bump()\n    rx_batch_end()\n}}"
            );
            return (body, Some(EventArgKind::I32("index".into())));
        }
        if let Some((item_var, field)) = split_field_access(arg) {
            if self.loop_stack.iter().any(|f| f.item_var == item_var) {
                let body = format!(
                    "micro {evt_name}(arg: utf8) {{\n    rx_batch_begin()\n    awsl_call_{fn_export}(arg)\n    store_bump()\n    rx_batch_end()\n}}"
                );
                let bind_expr = format!("{item_var}.{field}");
                return (body, Some(EventArgKind::Utf8(bind_expr)));
            }
        }
        let rewritten = self.rewrite_expr(arg);
        let body = format!(
            "micro {evt_name}() {{\n    rx_batch_begin()\n    awsl_call_{fn_export}({rewritten})\n    store_bump()\n    rx_batch_end()\n}}"
        );
        (body, None)
    }

    pub(crate) fn emit_roots(&mut self) -> String {
        self.emit_node_ids(&self.module.roots)
    }

    pub(crate) fn emit_roots_into(&mut self, container: &str) {
        for &node_id in &self.module.roots {
            let child = self.emit_node(node_id);
            writeln!(self.out, "    dom_append({container}, {child})").unwrap();
        }
    }

    fn emit_node_ids(&mut self, node_ids: &[RenderNodeId]) -> String {
        if node_ids.is_empty() {
            return self.emit_text_v("");
        }
        if node_ids.len() == 1 {
            return self.emit_node(node_ids[0]);
        }
        let frag = self.next_var();
        writeln!(self.out, "    let {frag} = dom_create_element(\"div\")").unwrap();
        for &node_id in node_ids {
            let child = self.emit_node(node_id);
            writeln!(self.out, "    dom_append({frag}, {child})").unwrap();
        }
        frag
    }

    pub(crate) fn emit_region_into(&mut self, region: RenderRegionId, container: &str) {
        for &node_id in region_nodes(self.module, region) {
            let child = self.emit_node(node_id);
            writeln!(self.out, "    dom_append({container}, {child})").unwrap();
        }
    }

    fn emit_node(&mut self, node_id: RenderNodeId) -> String {
        match self.module.node(node_id) {
            RenderNode::Fragment(fragment) => {
                let children = region_nodes(self.module, fragment.children);
                if children.len() == 1 {
                    return self.emit_node(children[0]);
                }
                let frag = self.next_var();
                writeln!(self.out, "    let {frag} = dom_create_element(\"div\")").unwrap();
                for &child_id in children {
                    let child = self.emit_node(child_id);
                    writeln!(self.out, "    dom_append({frag}, {child})").unwrap();
                }
                frag
            }
            RenderNode::Element(element) => {
                if let Some(intrinsic) = map_awsl_component_to_host(&element.tag) {
                    return self.emit_view_tag(intrinsic, &element.attrs, element.children);
                }
                self.emit_view_tag(&element.tag, &element.attrs, element.children)
            }
            RenderNode::Component(component) => self.emit_component_call(&component.tag, &component.attrs, component.children),
            RenderNode::Text(text) => self.emit_text_segments(&text.segments),
            RenderNode::If(render_if) => self.emit_if(render_if),
            RenderNode::Loop(render_loop) => self.emit_loop(render_loop),
        }
    }

    fn emit_if(&mut self, render_if: &RenderIfNode) -> String {
        let container = self.next_var();
        writeln!(self.out, "    let {container} = dom_create_element(\"div\")").unwrap();
        let condition = self.module.expr_source(render_if.condition);
        let cond_export = self.register_bool_expr(condition);
        let mount_export = self.register_mount_export(render_if.then_region);
        let (dep1, dep2) = self.dep_args(condition);
        writeln!(self.out, "    rx_bind_if({container}, \"{cond_export}\", \"{mount_export}\", {dep1}, {dep2})").unwrap();
        container
    }

    fn emit_loop(&mut self, render_loop: &RenderLoopNode) -> String {
        let container = self.next_var();
        writeln!(self.out, "    let {container} = dom_create_element(\"div\")").unwrap();
        let items_source = self.module.expr_source(render_loop.items);
        let items_rewrite = self.rewrite_expr(items_source);
        let items_export = self.register_list_expr(items_source);
        let mount_export = self.register_loop_mount_export(render_loop, &items_rewrite);
        let key_export = render_loop
            .key
            .map(|key_id| self.register_utf8_expr(self.module.expr_source(key_id)))
            .unwrap_or_else(|| "awsl_key_none".to_string());
        let (dep1, mut dep2) = self.dep_args(items_source);
        if region_uses_reactive(self.module, render_loop.body_region, self.bindings) && dep2 == "-1" {
            dep2 = "-2".into();
        }
        writeln!(self.out, "    rx_bind_loop({container}, \"{items_export}\", \"{mount_export}\", \"{key_export}\", {dep1}, {dep2})").unwrap();
        container
    }

    fn register_list_expr(&mut self, expr: &str) -> String {
        let name = self.next_export_id("items");
        let rewritten = self.rewrite_expr(expr);
        self.aux.push(format!("micro {name}(): list {{\n    return {rewritten}\n}}"));
        name
    }

    fn register_mount_export(&mut self, region: RenderRegionId) -> String {
        let name = self.next_export_id("mount");
        let mut body = String::new();
        let mut inner = VRenderCodegen {
            route: self.route,
            module: self.module,
            bindings: self.bindings,
            aux: self.aux,
            out: &mut body,
            var_counter: self.var_counter,
            loop_stack: self.loop_stack.clone(),
        };
        let child_ids = region_nodes(self.module, region);
        if child_ids.len() == 1 {
            if let RenderNode::Fragment(fragment) = self.module.node(child_ids[0]) {
                inner.emit_region_into(fragment.children, "container");
                self.var_counter = inner.var_counter;
                self.aux.push(format!("micro {name}(container: i32): i32 {{\n{body}    return container\n}}"));
                return name;
            }
        }
        let root = inner.emit_node_ids(child_ids);
        self.var_counter = inner.var_counter;
        writeln!(body, "    let mounted = {root}").unwrap();
        writeln!(body, "    dom_append(container, mounted)").unwrap();
        writeln!(body, "    return mounted").unwrap();
        self.aux.push(format!("micro {name}(container: i32): i32 {{\n{body}}}"));
        name
    }

    fn register_loop_mount_export(&mut self, render_loop: &RenderLoopNode, items_rewrite: &str) -> String {
        let name = self.next_export_id("loop_mount");
        let frame = LoopFrame {
            item_var: render_loop.item_var.clone(),
            index_var: render_loop.index_var.clone(),
            items_expr: items_rewrite.to_string(),
        };
        let mut mount_body = String::new();
        let mut stack = self.loop_stack.clone();
        stack.push(frame.clone());
        let mut inner = VRenderCodegen {
            route: self.route,
            module: self.module,
            bindings: self.bindings,
            aux: self.aux,
            out: &mut mount_body,
            var_counter: self.var_counter,
            loop_stack: stack,
        };
        let child = inner.emit_node_ids(region_nodes(self.module, render_loop.body_region));
        self.var_counter = inner.var_counter;
        let item_var = &frame.item_var;
        self.aux.push(format!(
            "micro {name}(container: i32, index: i32): i32 {{\n    let {item_var} = {items_rewrite}[index]\n{mount_body}    let child = {child}\n    dom_append(container, child)\n    return child\n}}"
        ));
        name
    }

    fn emit_view_tag(&mut self, tag: &str, attrs: &[RenderAttr], children: RenderRegionId) -> String {
        let host_tag = match tag {
            "Text" | "text" => "span",
            "Column" | "Box" | "Row" | "Flex" | "column" | "box" | "row" | "flex" => "div",
            "Button" | "button" => "button",
            other => other,
        };
        let var = self.next_var();
        writeln!(self.out, "    let {var} = dom_create_element(\"{host_tag}\")").unwrap();
        for attr in attrs {
            self.emit_attr(&var, attr);
        }
        for &child_id in region_nodes(self.module, children) {
            let child_var = self.emit_node(child_id);
            writeln!(self.out, "    dom_append({var}, {child_var})").unwrap();
        }
        var
    }

    fn emit_component_call(&mut self, tag: &str, attrs: &[RenderAttr], children: RenderRegionId) -> String {
        let export = hydrate_name_for_route(tag);
        let var = self.next_var();
        writeln!(self.out, "    let {var} = {export}()").unwrap();
        for attr in attrs {
            if attr.is_prop {
                self.emit_prop_bind(tag, attr);
            }
            else if !attr.is_event {
                self.emit_attr(&var, attr);
            }
        }
        for attr in attrs {
            if attr.is_event {
                self.emit_event_attr(&var, attr);
            }
        }
        for &child_id in region_nodes(self.module, children) {
            let child_var = self.emit_node(child_id);
            writeln!(self.out, "    dom_append({var}, {child_var})").unwrap();
        }
        var
    }

    fn register_utf8_literal(&mut self, text: &str) -> String {
        let name = self.next_export_id("expr");
        let escaped = escape_v_string(text);
        self.aux.push(format!("micro {name}(): utf8 {{\n    return \"{escaped}\"\n}}"));
        name
    }

    fn emit_prop_bind(&mut self, component: &str, attr: &RenderAttr) {
        let sig_export = format!("awsl_sig_{}_{}", sanitize_export(component), sanitize_export(&attr.name));
        let (expr_export, dep_source): (String, String) = match &attr.value {
            RenderAttrValue::Static(text) => (self.register_utf8_literal(text), String::new()),
            RenderAttrValue::Expr(expr_id) => {
                let source = self.module.expr_source(*expr_id).to_string();
                (self.register_utf8_expr(&source), source)
            }
            RenderAttrValue::Template(segments) => {
                let expr = segments
                    .iter()
                    .map(|segment| match segment {
                        RenderTextSegment::Static(text) => format!("\"{}\"", escape_v_string(text)),
                        RenderTextSegment::Expr(expr_id) => {
                            rewrite_utf8_expr_body(self.module.expr_source(*expr_id), self.bindings, &self.loop_stack)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" + ");
                let export = self.register_utf8_expr(&expr);
                (export, expr)
            }
        };
        let (dep1, dep2) = self.dep_args(&dep_source);
        writeln!(self.out, "    rx_bind_prop_utf8({sig_export}(), \"{expr_export}\", {dep1}, {dep2})").unwrap();
    }

    fn emit_attr(&mut self, handle: &str, attr: &RenderAttr) {
        if attr.is_event {
            self.emit_event_attr(handle, attr);
            return;
        }
        match &attr.value {
            RenderAttrValue::Static(text) => {
                writeln!(self.out, "    dom_set_attr({handle}, \"{}\", \"{text}\")", attr.name).unwrap();
            }
            RenderAttrValue::Expr(expr_id) => self.emit_dynamic_attr(handle, &attr.name, self.module.expr_source(*expr_id)),
            RenderAttrValue::Template(segments) => {
                let expr = segments
                    .iter()
                    .map(|segment| match segment {
                        RenderTextSegment::Static(text) => format!("\"{}\"", escape_v_string(text)),
                        RenderTextSegment::Expr(expr_id) => {
                            rewrite_utf8_expr_body(self.module.expr_source(*expr_id), self.bindings, &self.loop_stack)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" + ");
                if !self.loop_stack.is_empty() {
                    writeln!(self.out, "    dom_set_attr({handle}, \"{}\", {expr})", attr.name).unwrap();
                }
                else {
                    self.emit_dynamic_attr(handle, &attr.name, &expr);
                }
            }
        }
    }

    fn emit_dynamic_attr(&mut self, handle: &str, name: &str, expr: &str) {
        if !self.loop_stack.is_empty() {
            let body = rewrite_utf8_expr_body(expr, self.bindings, &self.loop_stack);
            writeln!(self.out, "    dom_set_attr({handle}, \"{name}\", {body})").unwrap();
            return;
        }
        let export = self.register_utf8_expr(expr);
        let (dep1, dep2) = self.dep_args(expr);
        if name == "class" {
            writeln!(self.out, "    rx_bind_class_utf8({handle}, \"{export}\", {dep1}, {dep2})").unwrap();
        }
        else {
            writeln!(self.out, "    rx_bind_attr_utf8({handle}, \"{name}\", \"{export}\", {dep1}, {dep2})").unwrap();
        }
    }

    fn emit_event_attr(&mut self, handle: &str, attr: &RenderAttr) {
        let handler = attr_value_source(self.module, &attr.value);
        let event = attr.name.strip_prefix("on:").or_else(|| attr.name.strip_prefix('@')).unwrap_or("click");
        let (export, arg_kind) = self.register_event(&handler);
        match arg_kind {
            Some(EventArgKind::Utf8(bind_expr)) => {
                let rewritten = self.rewrite_expr(&bind_expr);
                writeln!(self.out, "    dom_add_event_export_utf8({handle}, \"{event}\", \"{export}\", {rewritten})").unwrap();
            }
            Some(EventArgKind::I32(bind_expr)) => {
                writeln!(self.out, "    dom_add_event_export_i32({handle}, \"{event}\", \"{export}\", {bind_expr})").unwrap();
            }
            None => {
                writeln!(self.out, "    dom_add_event_export({handle}, \"{event}\", \"{export}\")").unwrap();
            }
        }
    }

    fn emit_text_segments(&mut self, segments: &[RenderTextSegment]) -> String {
        if segments.len() == 1 {
            return match &segments[0] {
                RenderTextSegment::Static(text) => self.emit_text_v(text),
                RenderTextSegment::Expr(expr_id) => self.emit_dynamic_text(self.module.expr_source(*expr_id)),
            };
        }
        let var = self.next_var();
        writeln!(self.out, "    let {var} = dom_create_element(\"span\")").unwrap();
        for segment in segments {
            let child = match segment {
                RenderTextSegment::Static(text) => self.emit_text_v(text),
                RenderTextSegment::Expr(expr_id) => self.emit_dynamic_text(self.module.expr_source(*expr_id)),
            };
            writeln!(self.out, "    dom_append({var}, {child})").unwrap();
        }
        var
    }

    fn emit_dynamic_text(&mut self, expr: &str) -> String {
        let var = self.next_var();
        if !self.loop_stack.is_empty() {
            let body = rewrite_utf8_expr_body(expr, self.bindings, &self.loop_stack);
            writeln!(self.out, "    let {var} = dom_create_text({body})").unwrap();
            return var;
        }
        writeln!(self.out, "    let {var} = dom_create_text(\"\")").unwrap();
        let export = self.register_utf8_expr(expr);
        let (dep1, dep2) = self.dep_args(expr);
        writeln!(self.out, "    rx_bind_text_utf8({var}, \"{export}\", {dep1}, {dep2})").unwrap();
        var
    }

    fn emit_text_v(&mut self, text: &str) -> String {
        let var = self.next_var();
        let escaped = escape_v_string(text);
        writeln!(self.out, "    let {var} = dom_create_text(\"{escaped}\")").unwrap();
        var
    }
}
