//! SSG 发射器：把准入子集内的 RenderIR 发射为调用 `asgard.render` 合同的 V 源码。
//!
//! 发射器不做任何表达式求值：E1 表达式经准入门归类后直出为 V 源码，
//! 语义由 V 编译器与 OVM 执行承担。多根组件统一包进透明片段容器
//! （`asgard:fragment`），由交付序列化层拆包，不在树中留下额外节点。

use std::fmt::Write as _;

use crate::asgard::{
    awsl::{
        LoweredComponent, RenderAttrValue, RenderModule, RenderNode, RenderNodeId, RenderTextSegment, ScriptBinding, TemplateNodeKind,
        render_ir::{IntrinsicWidget, region_nodes},
    },
    codegen::{
        ssg_ovm::admission::{self, E1Expr},
        v_render::util::{encode_v_text_literal, is_simple_identifier},
    },
    error::{AsgardError, AsgardResult},
};

/// 透明片段容器标签：多根组件的分组节点，序列化层拆包，不输出标签。
pub(crate) const FRAGMENT_TAG: &str = "asgard:fragment";

/// 发射组件的 SSG 渲染源码；调用方必须先通过准入门。
pub(crate) fn emit_ssg_source(component: &LoweredComponent) -> AsgardResult<String> {
    let mut emitter = Emitter {
        module: &component.render_ir,
        bindings: &component.script_bindings,
        lets: Vec::new(),
        body: String::new(),
        var_counter: 0,
        indent: 0,
    };
    emitter.emit_roots()?;
    let mut source = String::from("using asgard.render;\n\n[export]\nmicro awsl_ssg_render(): HostElement {\n");
    for line in &emitter.lets {
        writeln!(source, "    {line}").map_err(|error| AsgardError::msg(report!("组装 SSG 源码失败: {error}")))?;
    }
    source.push_str(&emitter.body);
    source.push_str("    return __ssg_h0\n}\n");
    Ok(source)
}

struct Emitter<'a> {
    module: &'a RenderModule,
    bindings: &'a [ScriptBinding],
    lets: Vec<String>,
    body: String,
    var_counter: u32,
    indent: usize,
}

impl Emitter<'_> {
    fn next_var(&mut self) -> String {
        let name = format!("__ssg_h{}", self.var_counter);
        self.var_counter += 1;
        name
    }

    fn line(&mut self, text: impl AsRef<str>) {
        let pad = "    ".repeat(self.indent + 1);
        writeln!(self.body, "{pad}{}", text.as_ref()).expect("SSG 发射写入内存字符串不可失败");
    }

    fn emit_roots(&mut self) -> AsgardResult<()> {
        let root = self.next_var();
        debug_assert_eq!(root, "__ssg_h0", "根容器必须是第一个变量");
        self.line(format!("let {root} = dom_create_element(\"{FRAGMENT_TAG}\")"));
        let roots = self.module.roots.clone();
        for &node_id in &roots {
            let child = self.emit_node(node_id)?;
            if let Some(child) = child {
                self.line(format!("dom_append({root}, {child})"));
            }
        }
        Ok(())
    }

    /// 发射一个节点；被跳过的节点（head/script/slot）返回 `None`。
    fn emit_node(&mut self, node_id: RenderNodeId) -> AsgardResult<Option<String>> {
        match self.module.node(node_id) {
            RenderNode::Element(element) => {
                let tag = element.tag.as_str();
                if tag.eq_ignore_ascii_case("head") || tag.eq_ignore_ascii_case("script") {
                    return Ok(None);
                }
                if matches!(element.kind, TemplateNodeKind::Intrinsic) && tag.eq_ignore_ascii_case("slot") {
                    return Ok(None);
                }
                let host_tag = map_host_tag(element.widget, tag, element.kind);
                let var = self.next_var();
                self.line(format!("let {var} = dom_create_element({})", encode_v_text_literal(&host_tag)));
                for attr in &element.attrs {
                    if attr.is_event {
                        continue;
                    }
                    let Some(name) = normalize_attr_name(&attr.name)
                    else {
                        continue;
                    };
                    let value = self.attr_value(&attr.value)?;
                    self.line(format!("dom_set_attr({var}, {}, {value})", encode_v_text_literal(&name)));
                }
                let children = region_nodes(self.module, element.children).to_vec();
                for child_id in children {
                    if let Some(child) = self.emit_node(child_id)? {
                        self.line(format!("dom_append({var}, {child})"));
                    }
                }
                Ok(Some(var))
            }
            RenderNode::Text(text) => {
                if text.segments.len() == 1 {
                    let value = match &text.segments[0] {
                        RenderTextSegment::Static(text) => encode_v_text_literal(text),
                        RenderTextSegment::Expr(expr_id) => {
                            let source = self.module.expr_source(*expr_id).to_string();
                            self.expr_value(&source)?
                        }
                    };
                    let var = self.next_var();
                    self.line(format!("let {var} = dom_create_text({value})"));
                    return Ok(Some(var));
                }
                // 多段文本：逐段一个文本节点，装入透明片段容器；
                // 序列化拆包后与迁移前的连续文本逐字节一致。
                let frag = self.next_var();
                self.line(format!("let {frag} = dom_create_element(\"{FRAGMENT_TAG}\")"));
                for segment in &text.segments {
                    let value = match segment {
                        RenderTextSegment::Static(text) => encode_v_text_literal(text),
                        RenderTextSegment::Expr(expr_id) => {
                            let source = self.module.expr_source(*expr_id).to_string();
                            self.expr_value(&source)?
                        }
                    };
                    let var = self.next_var();
                    self.line(format!("let {var} = dom_create_text({value})"));
                    self.line(format!("dom_append({frag}, {var})"));
                }
                return Ok(Some(frag));
            }
            RenderNode::Fragment(fragment) => {
                let var = self.next_var();
                self.line(format!("let {var} = dom_create_element(\"{FRAGMENT_TAG}\")"));
                let children = region_nodes(self.module, fragment.children).to_vec();
                for child_id in children {
                    if let Some(child) = self.emit_node(child_id)? {
                        self.line(format!("dom_append({var}, {child})"));
                    }
                }
                Ok(Some(var))
            }
            RenderNode::If(render_if) => {
                // 条件经准入门校验为 E1 bool：预建透明容器，分支体内把内容挂进容器，
                // 由 OVM 按组件初始状态静态取分支。用 if 语句而非 if 表达式，
                // 与编译器已验证的语句形态保持一致。
                let condition = self.module.expr_source(render_if.condition).to_string();
                let condition_value = self.expr_value(&condition)?;
                let var = self.next_var();
                self.line(format!("let {var} = dom_create_element(\"{FRAGMENT_TAG}\")"));
                self.line(format!("if {condition_value} {{"));
                self.emit_if_branch(render_if.then_region, &var)?;
                // 空 else 不发射分支块，避免产出空语句块。
                if !region_nodes(self.module, render_if.else_region).is_empty() {
                    self.line("} else {");
                    self.emit_if_branch(render_if.else_region, &var)?;
                }
                self.line("}");
                Ok(Some(var))
            }
            RenderNode::Loop(_) | RenderNode::Component(_) => {
                Err(AsgardError::msg(report!("SSG 准入门放行后发射器遇到越界节点: {:?}", self.module.node(node_id))))
            }
        }
    }

    /// 发射 `if` 分支体：比外层深一级缩进，把分支内节点逐个挂进 `parent` 容器。
    fn emit_if_branch(&mut self, region: crate::asgard::awsl::RenderRegionId, parent: &str) -> AsgardResult<()> {
        self.indent += 1;
        let children = region_nodes(self.module, region).to_vec();
        for child_id in children {
            if let Some(child) = self.emit_node(child_id)? {
                self.line(format!("dom_append({parent}, {child})"));
            }
        }
        self.indent -= 1;
        Ok(())
    }

    fn attr_value(&mut self, value: &RenderAttrValue) -> AsgardResult<String> {
        match value {
            RenderAttrValue::Static(text) => Ok(encode_v_text_literal(text)),
            RenderAttrValue::Expr(expr_id) => self.expr_value(self.module.expr_source(*expr_id)),
            RenderAttrValue::Template(segments) => self.text_value(segments),
        }
    }

    fn text_value(&mut self, segments: &[RenderTextSegment]) -> AsgardResult<String> {
        if segments.len() == 1 {
            return match &segments[0] {
                RenderTextSegment::Static(text) => Ok(encode_v_text_literal(text)),
                RenderTextSegment::Expr(expr_id) => {
                    let source = self.module.expr_source(*expr_id).to_string();
                    self.expr_value(&source)
                }
            };
        }
        let mut merged = String::new();
        for segment in segments {
            match segment {
                RenderTextSegment::Static(text) => merged.push_str(text),
                RenderTextSegment::Expr(_) => {
                    return Err(AsgardError::msg(report!("SSG 准入门放行后发射器遇到混合段文本")));
                }
            }
        }
        Ok(encode_v_text_literal(&merged))
    }

    fn expr_value(&mut self, source: &str) -> AsgardResult<String> {
        match admission::classify_e1(source, self.bindings) {
            Ok(E1Expr::Literal(literal)) => Ok(literal),
            Ok(E1Expr::Binding(name)) => {
                self.register_binding_let(&name)?;
                Ok(name)
            }
            Err(diagnostic) => Err(AsgardError::msg(report!("SSG 准入门放行后发射器遇到越界表达式: {diagnostic}"))),
        }
    }

    fn register_binding_let(&mut self, name: &str) -> AsgardResult<()> {
        if self.lets.iter().any(|line| line.starts_with(&format!("let {name} ="))) {
            return Ok(());
        }
        let binding = self
            .bindings
            .iter()
            .find(|binding| binding.name == name)
            .ok_or_else(|| AsgardError::msg(report!("SSG 发射器引用缺失绑定 `{name}`")))?;
        if !is_simple_identifier(&binding.name) {
            return Err(AsgardError::msg(report!("SSG 绑定名不是合法标识符: `{}`", binding.name)));
        }
        self.lets.push(format!("let {name} = {}", binding.init_expr.trim()));
        Ok(())
    }
}

/// 标签投影：与 HTML 发射器共用 [`IntrinsicWidget::html_element`] 的 Web 宿主事实。
///
/// 组件识别与语义不在这里（组件构造已被准入门拒绝）；只按 RenderIR 已冻结的
/// intrinsic 身份选择宿主元素。
fn map_host_tag(widget: Option<IntrinsicWidget>, tag: &str, kind: TemplateNodeKind) -> String {
    match kind {
        TemplateNodeKind::Intrinsic => widget.map_or_else(|| tag.to_ascii_lowercase(), |widget| widget.html_element().to_string()),
        TemplateNodeKind::HostView | TemplateNodeKind::Component => tag.to_ascii_lowercase(),
    }
}

/// 属性名归一：`:x` 去前缀；`@` / `on:` 事件形态返回 `None`（不进入静态属性）。
fn normalize_attr_name(name: &str) -> Option<String> {
    match name {
        n if n.starts_with(':') => Some(n[1..].to_string()),
        n if n.starts_with('@') => None,
        n if n == "on:click" || n.starts_with("on:") => None,
        n => Some(n.to_string()),
    }
}
