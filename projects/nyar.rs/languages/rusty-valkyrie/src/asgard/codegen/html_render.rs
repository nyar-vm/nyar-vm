//! RenderIR → 静态 HTML（SSG / 文档站，无 hydration）。
//!
//! **退役中**：静态交付（`static_site`）已切换到 `codegen::ssg_ovm` 的
//! V/OVM 渲染路径；本模块仅作为迁移期对照实现服务其余消费方（文档/报告
//! 与既有测试），新逻辑不得再加到这里，全部消费方迁移后随 `expr_eval`
//! 一并删除。

use std::fmt::Write as _;

use oak_awsl::is_html_void_element;
use serde_json::Value;

use crate::asgard::{
    awsl::{
        LoweredComponent, RenderAttr, RenderAttrValue, RenderLoopNode, RenderModule, RenderNode, RenderNodeId, RenderRegionId,
        RenderTextSegment, ScriptBinding, TemplateNodeKind,
        render_ir::{attr_value_source, region_nodes},
    },
    codegen::expr_eval::{EvalDiagnostic, EvalDiagnosticKind, ExprContext},
};

/// RenderIR 节点嵌套渲染上限。超过上限记录结构化诊断并跳过子树，
/// 不允许深嵌套模板把宿主进程栈打穿。
pub const MAX_RENDER_NESTING_DEPTH: usize = 64;

/// 静态 HTML 渲染结果。
#[derive(Debug, Clone)]
pub struct StaticRenderResult {
    /// 组件 HTML 片段（含根 `div` 包装）。
    pub html: String,
    /// 作用域 CSS。
    pub css: String,
    /// 作用域 class 前缀。
    pub scope: String,
    /// 渲染期结构化诊断（表达式求值深度、节点嵌套深度）。
    pub diagnostics: Vec<EvalDiagnostic>,
}

/// 将已降级的组件 RenderIR 渲染为静态 HTML。
pub fn render_static_ir(component: &LoweredComponent, data: &Value) -> StaticRenderResult {
    let scope = format!("asgard-static-{}", to_kebab_case(&component.name));
    let mut html_body = String::new();
    let mut ctx = HtmlRenderCtx::new(component, data, &scope);
    ctx.emit_roots(&mut html_body, &component.render_ir);

    let css = component.style.as_deref().unwrap_or("").to_string();
    let html = format!(
        "<div class=\"{scope}\" data-asgard-static=\"{name}\">\n{html_body}\n</div>",
        scope = scope,
        name = escape_attr(&component.route_name),
        html_body = html_body.trim_end()
    );

    let diagnostics = ctx.expr.diagnostics();
    StaticRenderResult { html, css, scope, diagnostics }
}

struct HtmlRenderCtx<'a> {
    module: &'a RenderModule,
    expr: ExprContext,
    scope: &'a str,
}

impl<'a> HtmlRenderCtx<'a> {
    fn new(component: &'a LoweredComponent, data: &Value, scope: &'a str) -> Self {
        Self { module: &component.render_ir, expr: ExprContext::from_bindings(&component.script_bindings, data), scope }
    }

    fn emit_roots(&mut self, out: &mut String, module: &RenderModule) {
        self.emit_region(out, module, &module.roots, 0);
    }

    fn emit_region(&mut self, out: &mut String, module: &RenderModule, node_ids: &[RenderNodeId], depth: usize) {
        for &node_id in node_ids {
            self.emit_node(out, module, node_id, depth);
        }
    }

    fn emit_region_id(&mut self, out: &mut String, module: &RenderModule, region: RenderRegionId, depth: usize) {
        self.emit_region(out, module, region_nodes(module, region), depth);
    }

    fn emit_node(&mut self, out: &mut String, module: &RenderModule, node_id: RenderNodeId, depth: usize) {
        if depth > MAX_RENDER_NESTING_DEPTH {
            self.expr.record_diagnostic(EvalDiagnostic {
                kind: EvalDiagnosticKind::RenderNestingExceeded,
                target: node_label(module, node_id),
                depth,
                limit: MAX_RENDER_NESTING_DEPTH,
            });
            return;
        }
        match module.node(node_id) {
            RenderNode::Element(element) => {
                let tag = &element.tag;
                let kind = element.kind;
                if matches!(kind, TemplateNodeKind::Intrinsic) && tag.eq_ignore_ascii_case("slot") {
                    return;
                }
                if tag.eq_ignore_ascii_case("head") || tag.eq_ignore_ascii_case("script") {
                    return;
                }
                let html_tag = map_html_tag(tag, kind);
                write!(out, "<{html_tag}").unwrap();
                self.emit_attrs(out, module, &element.attrs);
                let children = region_nodes(module, element.children);
                if children.is_empty() && is_html_void_element(&html_tag) {
                    write!(out, " />").unwrap();
                }
                else {
                    write!(out, ">").unwrap();
                    self.emit_region(out, module, children, depth + 1);
                    write!(out, "</{html_tag}>").unwrap();
                }
            }
            RenderNode::Component(component) => {
                let html_tag = map_html_tag(&component.tag, TemplateNodeKind::Component);
                write!(out, "<{html_tag}").unwrap();
                self.emit_attrs(out, module, &component.attrs);
                write!(out, ">").unwrap();
                self.emit_region_id(out, module, component.children, depth + 1);
                write!(out, "</{html_tag}>").unwrap();
            }
            RenderNode::Text(text) => {
                let body = self.emit_text_segments(module, &text.segments);
                out.push_str(&escape_html(&body));
            }
            RenderNode::If(render_if) => {
                if self.expr.eval_truthy(module.expr_source(render_if.condition)) {
                    self.emit_region_id(out, module, render_if.then_region, depth + 1);
                }
                else {
                    self.emit_region_id(out, module, render_if.else_region, depth + 1);
                }
            }
            RenderNode::Loop(render_loop) => self.emit_loop(out, module, render_loop, depth + 1),
            RenderNode::Fragment(fragment) => self.emit_region_id(out, module, fragment.children, depth + 1),
        }
    }

    fn emit_loop(&mut self, out: &mut String, module: &RenderModule, render_loop: &RenderLoopNode, body_depth: usize) {
        let items = self.expr.eval_value(module.expr_source(render_loop.items));
        let Some(array) = items.as_array()
        else {
            return;
        };
        for (index, item) in array.iter().enumerate() {
            self.expr.push_loop(&render_loop.item_var, &render_loop.index_var, item.clone(), index);
            self.emit_region_id(out, module, render_loop.body_region, body_depth);
            self.expr.pop_loop();
        }
    }

    fn emit_attrs(&self, out: &mut String, module: &RenderModule, attrs: &[RenderAttr]) {
        for attr in attrs {
            if attr.is_event {
                continue;
            }
            let name = normalize_attr_name(&attr.name);
            if name.is_empty() {
                continue;
            }
            let value = match &attr.value {
                RenderAttrValue::Static(text) => text.clone(),
                RenderAttrValue::Expr(expr_id) => self.expr.eval_string(module.expr_source(*expr_id)),
                RenderAttrValue::Template(segments) => self.emit_text_segments(module, segments),
            };
            if !value.is_empty() || name == "class" || name == "href" {
                write!(out, " {name}=\"{}\"", escape_attr(&value)).unwrap();
            }
        }
    }

    fn emit_text_segments(&self, module: &RenderModule, segments: &[RenderTextSegment]) -> String {
        let mut out = String::new();
        for segment in segments {
            match segment {
                RenderTextSegment::Static(text) => out.push_str(text),
                RenderTextSegment::Expr(expr_id) => out.push_str(&self.expr.eval_string(module.expr_source(*expr_id))),
            }
        }
        out
    }
}

fn node_label(module: &RenderModule, node_id: RenderNodeId) -> String {
    match module.node(node_id) {
        RenderNode::Element(element) => element.tag.clone(),
        RenderNode::Component(component) => component.tag.clone(),
        RenderNode::Text(_) => "text".into(),
        RenderNode::If(_) => "if".into(),
        RenderNode::Loop(_) => "loop".into(),
        RenderNode::Fragment(_) => "fragment".into(),
    }
}

fn map_html_tag(tag: &str, kind: TemplateNodeKind) -> String {
    match kind {
        TemplateNodeKind::Component if tag.eq_ignore_ascii_case("link") => "a".into(),
        TemplateNodeKind::Component => "div".into(),
        TemplateNodeKind::Intrinsic => match tag {
            "Text" | "text" => "span".into(),
            "Button" | "button" => "button".into(),
            "Column" | "column" | "Box" | "box" | "Row" | "row" | "Flex" | "flex" => "div".into(),
            "Slot" | "slot" => "div".into(),
            other => other.to_ascii_lowercase(),
        },
        TemplateNodeKind::HostView => tag.to_ascii_lowercase(),
    }
}

fn normalize_attr_name(name: &str) -> String {
    match name {
        "href" | "class" | "id" | "title" | "rel" | "target" | "type" | "name" | "content" => name.to_string(),
        n if n.starts_with(':') => n[1..].to_string(),
        n if n.starts_with('@') => String::new(),
        n if n == "on:click" || n.starts_with("on:") => String::new(),
        n => n.to_string(),
    }
}

fn to_kebab_case(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        }
        else if ch == '_' {
            out.push('-');
        }
        else {
            out.push(ch);
        }
    }
    out
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn escape_attr(text: &str) -> String {
    escape_html(text).replace('"', "&quot;")
}
