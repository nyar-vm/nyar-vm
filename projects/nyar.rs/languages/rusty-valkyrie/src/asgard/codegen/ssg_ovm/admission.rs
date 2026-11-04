//! SSG 准入门：判定组件 RenderIR 是否落在当前已迁移的构造子集内。
//!
//! 越出子集不是错误输入的兜底，而是迁移边界的显式裁决：每个被拒构造
//! 产出一条结构化诊断，`static_site` 据此失败闭合，不写残缺产物。
//! 准入门只在 lowering 之后读取已降级的 RenderIR 与 script 绑定，
//! 不做任何表达式求值——表达式语义由 V 编译器与 OVM 执行承担。

use crate::asgard::{
    awsl::{
        LoweredComponent, RenderAttrValue, RenderModule, RenderNode, RenderNodeId, RenderRegionId, RenderTextSegment, ScriptBinding,
        SignalValueType,
        render_ir::{RenderIfNode, region_nodes},
    },
    codegen::v_render::{expr::normalize_js_string_literals, util::is_simple_identifier},
};

/// 模板嵌套准入上限：越限子树不再展开，只报诊断。
/// 与迁移期安全网 `MAX_RENDER_NESTING_DEPTH` 同值，退役模块删除后由本常量承接。
pub const MAX_SSG_NESTING_DEPTH: usize = 64;

/// 准入拒绝的规则种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionRule {
    /// 构造尚未迁移（`<if>` / `<loop>` / 组件调用等）。
    OutOfSubsetConstruct,
    /// 表达式形态越出 E1（运算符、三元、函数调用、字段访问等）。
    UnsupportedExpression,
    /// 字符串拼接（多段文本 / 多段属性模板）尚未迁移。
    UnsupportedConcat,
    /// `isActive(...)` 模板魔法函数：任何时期显式拒绝，不静默吞掉。
    IsActiveRejected,
    /// 未解析的裸标识符：V 语义下是编译期错误，不返回空值。
    UnknownIdentifier,
    /// 绑定初始表达式越出 E1（聚合、运算、调用；聚合属类型系统缺口）。
    UnsupportedBindingInit,
    /// 必填组件属性缺省值缺失，SSG 无从求值。
    MissingRequiredPropertyDefault,
    /// 模板嵌套超过准入上限。
    NestingDepthExceeded,
    /// `each=`/`：each=` 魔法属性迭代写法：不是语言构造，任何时期硬拒绝。
    /// 迭代只有 `<loop item="x" :each="{items}">` 一种语言形态。
    MagicIterationAttribute,
}

impl AdmissionRule {
    /// 诊断标签（稳定文本，供测试与日志断言）。
    pub fn label(self) -> &'static str {
        match self {
            Self::OutOfSubsetConstruct => "构造越出 SSG 准入子集",
            Self::UnsupportedExpression => "表达式形态越出 SSG 准入子集",
            Self::UnsupportedConcat => "字符串拼接尚未迁移",
            Self::IsActiveRejected => "`isActive(...)` 已被显式拒绝",
            Self::UnknownIdentifier => "未解析标识符",
            Self::UnsupportedBindingInit => "绑定初始表达式越出 SSG 准入子集",
            Self::MissingRequiredPropertyDefault => "必填属性缺省值缺失",
            Self::NestingDepthExceeded => "模板嵌套超过准入上限",
            Self::MagicIterationAttribute => "不是语言构造",
        }
    }
}

/// 一条准入诊断。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmissionDiagnostic {
    /// 规则。
    pub rule: AdmissionRule,
    /// 触发对象（标签名、表达式源码或绑定名）。
    pub target: String,
}

impl std::fmt::Display for AdmissionDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.rule.label(), self.target)
    }
}

/// E1 表达式：字面量（V 源码直出）或已解析绑定引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum E1Expr {
    /// 归一化后的 V 字面量源码（字符串 / 整数 / 浮点 / 布尔）。
    Literal(String),
    /// 已解析的绑定名（发射为同名 `let` 的引用）。
    Binding(String),
}

/// 对组件执行准入门；空向量表示组件落在准入子集内。
pub fn check_component(component: &LoweredComponent) -> Vec<AdmissionDiagnostic> {
    let mut gate = Gate { module: &component.render_ir, bindings: &component.script_bindings, diagnostics: Vec::new(), referenced: Vec::new() };
    let roots = component.render_ir.roots.clone();
    for &root in &roots {
        gate.check_node(root, 0);
    }
    gate.validate_referenced_bindings();
    gate.diagnostics
}

struct Gate<'a> {
    module: &'a RenderModule,
    bindings: &'a [ScriptBinding],
    diagnostics: Vec<AdmissionDiagnostic>,
    referenced: Vec<String>,
}

impl Gate<'_> {
    fn reject(&mut self, rule: AdmissionRule, target: impl Into<String>) {
        let target = target.into();
        if !self.diagnostics.iter().any(|diag| diag.rule == rule && diag.target == target) {
            self.diagnostics.push(AdmissionDiagnostic { rule, target });
        }
    }

    fn check_region(&mut self, region: RenderRegionId, depth: usize) {
        for &node in region_nodes(self.module, region) {
            self.check_node(node, depth);
        }
    }

    fn check_node(&mut self, node_id: RenderNodeId, depth: usize) {
        if depth > MAX_SSG_NESTING_DEPTH {
            let target = match self.module.node(node_id) {
                RenderNode::Element(element) => element.tag.clone(),
                RenderNode::Component(component) => component.tag.clone(),
                RenderNode::Text(_) => "text".to_string(),
                RenderNode::If(_) => "if".to_string(),
                RenderNode::Loop(_) => "loop".to_string(),
                RenderNode::Fragment(_) => "fragment".to_string(),
            };
            self.reject(AdmissionRule::NestingDepthExceeded, target);
            return;
        }
        match self.module.node(node_id) {
            RenderNode::Element(element) => {
                for attr in &element.attrs {
                    // `each`/`：each` 魔法属性迭代写法不是语言构造：任何时期硬拒绝，点名唯一合法形态。
                    if attr.name == "each" || attr.name == ":each" {
                        self.reject(
                            AdmissionRule::MagicIterationAttribute,
                            format!(
                                "`{}` 上的 `{}` 属性（迭代只有 `<loop item=\"x\" :each=\"{{items}}\">` 一种语言形态）",
                                element.tag, attr.name
                            ),
                        );
                        continue;
                    }
                    if attr.is_event {
                        continue;
                    }
                    match &attr.value {
                        RenderAttrValue::Static(_) => {}
                        RenderAttrValue::Expr(expr_id) => {
                            self.check_expr(self.module.expr_source(*expr_id));
                        }
                        RenderAttrValue::Template(segments) => self.check_segments(segments, "attr"),
                    }
                }
                self.check_region(element.children, depth + 1);
            }
            RenderNode::Text(text) => {
                // 静态与插值在 RenderIR 中是兄弟文本节点，逐段各自判定即可，
                // 不构成字符串拼接（拼接只发生在属性模板值里）。
                for segment in &text.segments {
                    if let RenderTextSegment::Expr(expr_id) = segment {
                        self.check_expr(self.module.expr_source(*expr_id));
                    }
                }
            }
            RenderNode::Fragment(fragment) => self.check_region(fragment.children, depth + 1),
            RenderNode::If(render_if) => self.check_if_node(render_if, depth),
            RenderNode::Loop(_) => self.reject(
                AdmissionRule::OutOfSubsetConstruct,
                "loop（合法语言构造；其 SSG 降低当前被编译器执行通道缺口阻挡：数组下标在循环合流处的 \
                 类型身份不一致。聚合与迭代形态在通道修复后放行，勿改写为属性魔法迭代）",
            ),
            RenderNode::Component(component) => {
                if component.attrs.iter().any(|attr| attr.name == "each" || attr.name == ":each") {
                    self.reject(
                        AdmissionRule::MagicIterationAttribute,
                        format!(
                            "component `{}` 上的 `each` 属性（迭代只有 `<loop item=\"x\" :each=\"{{items}}\">` 一种语言形态）",
                            component.tag
                        ),
                    );
                    return;
                }
                self.reject(AdmissionRule::OutOfSubsetConstruct, format!("component `{}`", component.tag));
            }
        }
    }

    /// `<if>`/`<else>` 条件渲染：条件必须是 E1 表达式且求值为 `bool`
    ///（`true`/`false` 字面量或 `bool` 类型绑定），两个分支按同一子集递归校验。
    /// 其它类型的条件真值语义尚未进入合同，拒绝时点名原因与可用替代。
    fn check_if_node(&mut self, render_if: &RenderIfNode, depth: usize) {
        let condition = self.module.expr_source(render_if.condition);
        self.check_expr(condition);
        match classify_e1(condition, self.bindings) {
            Ok(E1Expr::Literal(literal)) => {
                if literal != "true" && literal != "false" {
                    self.reject(
                        AdmissionRule::OutOfSubsetConstruct,
                        format!("if 条件 `{condition}`（条件必须求值为 `bool`：`true`/`false` 字面量或 `bool` 类型绑定）"),
                    );
                }
            }
            Ok(E1Expr::Binding(name)) => {
                if let Some(binding) = self.bindings.iter().find(|binding| binding.name == name) {
                    if binding.value_type != SignalValueType::Bool {
                        self.reject(
                            AdmissionRule::OutOfSubsetConstruct,
                            format!(
                                "if 条件 `{condition}`（绑定 `{name}` 不是 `bool` 类型；其它类型的真值语义尚未进入合同，请改用 bool 绑定）"
                            ),
                        );
                    }
                }
            }
            Err(_) => {}
        }
        self.check_region(render_if.then_region, depth + 1);
        self.check_region(render_if.else_region, depth + 1);
    }

    fn check_segments(&mut self, segments: &[RenderTextSegment], what: &str) {
        let dynamic = segments.iter().filter(|segment| matches!(segment, RenderTextSegment::Expr(_))).count();
        if dynamic > 0 && segments.len() > 1 {
            self.reject(AdmissionRule::UnsupportedConcat, format!("{what} 混合静态与插值段"));
            return;
        }
        for segment in segments {
            if let RenderTextSegment::Expr(expr_id) = segment {
                self.check_expr(self.module.expr_source(*expr_id));
            }
        }
    }

    fn check_expr(&mut self, source: &str) {
        match classify_e1(source, self.bindings) {
            Ok(E1Expr::Binding(name)) => {
                if !self.referenced.contains(&name) {
                    self.referenced.push(name);
                }
            }
            Ok(E1Expr::Literal(_)) => {}
            Err(diagnostic) => self.diagnostics.push(diagnostic),
        }
    }

    fn validate_referenced_bindings(&mut self) {
        let referenced = std::mem::take(&mut self.referenced);
        for name in referenced {
            let Some(binding) = self.bindings.iter().find(|binding| binding.name == name)
            else {
                self.reject(AdmissionRule::UnknownIdentifier, name);
                continue;
            };
            if binding.init_expr.trim().is_empty() {
                self.reject(AdmissionRule::MissingRequiredPropertyDefault, name);
                continue;
            }
            if !is_e1_literal(&binding.init_expr) {
                self.reject(AdmissionRule::UnsupportedBindingInit, format!("`{}` = {}", name, binding.init_expr.trim()));
            }
        }
    }
}

/// 将模板表达式归入 E1；越出即给出结构化诊断。
pub(crate) fn classify_e1(source: &str, bindings: &[ScriptBinding]) -> Result<E1Expr, AdmissionDiagnostic> {
    let expr = source.trim().trim_start_matches('{').trim_end_matches('}').trim();
    if expr.starts_with("isActive(") {
        return Err(AdmissionDiagnostic { rule: AdmissionRule::IsActiveRejected, target: source.trim().to_string() });
    }
    if is_e1_literal(expr) {
        return Ok(E1Expr::Literal(normalize_literal(expr)));
    }
    if is_simple_identifier(expr) {
        if bindings.iter().any(|binding| binding.name == expr) {
            return Ok(E1Expr::Binding(expr.to_string()));
        }
        return Err(AdmissionDiagnostic { rule: AdmissionRule::UnknownIdentifier, target: expr.to_string() });
    }
    Err(AdmissionDiagnostic { rule: AdmissionRule::UnsupportedExpression, target: source.trim().to_string() })
}

/// E1 字面量判定：引号字符串、整数、浮点或布尔。
fn is_e1_literal(expr: &str) -> bool {
    let expr = expr.trim();
    if expr.len() >= 2 && ((expr.starts_with('"') && expr.ends_with('"')) || (expr.starts_with('\'') && expr.ends_with('\''))) {
        return true;
    }
    if expr == "true" || expr == "false" {
        return true;
    }
    expr.parse::<i64>().is_ok() || expr.parse::<f64>().is_ok()
}

/// 归一化字面量为 V 源码形态：单引号字符串转双引号，其余原样。
fn normalize_literal(expr: &str) -> String {
    normalize_js_string_literals(expr.trim())
}
