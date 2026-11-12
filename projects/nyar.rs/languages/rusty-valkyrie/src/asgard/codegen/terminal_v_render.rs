//! RenderIR → 终端 V 控件树构造函数（带 session 参数，产品 `awsl_*` 原语）。
//!
//! 终端路径消费冻结的 `std.terminal` 会话外形：控件树由产品模块 `asgard.terminal`
//! 的 `awsl_*` 原语构造；交互状态（布尔、单选组）以显示名经当前运行帧换取类型化槽
//! 句柄（`FlagBinding` / `ChoiceBinding`）；事件行为是函数值
//! `micro(mut TuiSession) -> unit`，构造控件时直接传入；纯展示文本在构造期直接求值
//! 写入 `Text` 控件。不存在绑定字符串登记、按名分派或全局运行时读写通道。本模块从
//! RenderIR 生成：
//!
//! - `awsl_build_<route>(session: mut TuiSession) -> ArrayList<Widget>`：带 session 的控件树构造
//! - `awsl_call_<handler>(session: mut TuiSession) -> unit`：事件处理函数（函数值回调）
//! - `awsl_main()`：把控件树构造函数以函数值交给产品入口 `asgard_terminal_run`

use std::fmt::Write as _;

use crate::asgard::awsl::{
    LoweredComponent, RenderAttr, RenderAttrValue, RenderIfNode, RenderLoopNode, RenderModule, RenderNode, RenderNodeId, RenderRegionId,
    RenderTextSegment, ScriptBinding, SignalValueType,
    render_ir::{attr_value_source, region_nodes},
};

use super::v_render::util::{encode_v_text_literal, replace_ident, strip_expr_braces};

/// 将组件 RenderIR 生成为终端 V 源码（session 控件树构造 + 事件处理函数 + main）。
pub fn render_terminal_component_v(component: &LoweredComponent) -> String {
    let route = sanitize(&component.route_name);
    let mut out = String::new();
    let mut ctx = TerminalVCodegen {
        route: &route,
        module: &component.render_ir,
        bindings: &component.script_bindings,
        var_counter: 0,
        handler_names: Vec::new(),
        needs_noop: false,
    };
    out.push_str(&emit_build(&mut ctx));
    out.push_str(&emit_handlers(&ctx.handler_names));
    if ctx.needs_noop {
        out.push_str(&emit_noop(&route));
    }
    out.push_str(&emit_main(&route));
    out
}

struct TerminalVCodegen<'a> {
    route: &'a str,
    module: &'a RenderModule,
    bindings: &'a [ScriptBinding],
    var_counter: u32,
    handler_names: Vec<String>,
    needs_noop: bool,
}

impl<'a> TerminalVCodegen<'a> {
    fn next_var(&mut self) -> String {
        let name = format!("w{}", self.var_counter);
        self.var_counter += 1;
        name
    }

    fn collect_handler(&mut self, name: &str) -> String {
        let full = format!("awsl_call_{name}");
        if !self.handler_names.contains(&full) {
            self.handler_names.push(full.clone());
        }
        full
    }

    /// 事件回调函数值：有 `@click` 时引用生成的处理函数，缺事件时落到组件级空操作
    /// 回调；控件行为字段是函数值类型，不存在空字符串占位通道。
    fn event_callback(&mut self, event: &str) -> String {
        if event.is_empty() {
            self.needs_noop = true;
            return format!("awsl_noop_{}", self.route);
        }
        self.collect_handler(event)
    }
}

/// 生成 `awsl_build_<route>(session)`：带 session 的控件树构造。
/// 交互控件在这里经当前运行帧换取类型化槽句柄，纯展示文本直接求值。
fn emit_build(ctx: &mut TerminalVCodegen<'_>) -> String {
    let mut out = String::new();
    writeln!(out, "# 控件树构造：接收 session；交互状态经当前运行帧换取类型化槽句柄，展示文本直接求值。").unwrap();
    writeln!(out, "micro awsl_build_{}(session: mut TuiSession) -> ArrayList<Widget> {{", ctx.route).unwrap();
    writeln!(out, "    let roots: ArrayList<Widget> = ArrayList::new(16)").unwrap();
    for &root_id in &ctx.module.roots {
        let var = emit_node(ctx, &mut out, root_id, "    ");
        if !var.is_empty() {
            writeln!(out, "    roots.push({var})").unwrap();
        }
    }
    writeln!(out, "    return roots").unwrap();
    writeln!(out, "}}\n").unwrap();
    out
}

/// 生成单个节点，返回变量名（空字符串表示不需要 push）。
fn emit_node(ctx: &mut TerminalVCodegen<'_>, out: &mut String, node_id: RenderNodeId, indent: &str) -> String {
    match ctx.module.node(node_id) {
        RenderNode::Element(element) => emit_tag(ctx, out, &element.tag, &element.attrs, element.children, indent),
        RenderNode::Component(component) => emit_tag(ctx, out, &component.tag, &component.attrs, component.children, indent),
        RenderNode::Text(text) => emit_text(ctx, out, &text.segments, indent),
        RenderNode::Fragment(fragment) => emit_fragment(ctx, out, fragment.children, indent),
        RenderNode::If(render_if) => emit_if(ctx, out, render_if, indent),
        RenderNode::Loop(render_loop) => emit_loop(ctx, out, render_loop, indent),
    }
}

fn emit_tag(
    ctx: &mut TerminalVCodegen<'_>,
    out: &mut String,
    tag: &str,
    attrs: &[RenderAttr],
    children: RenderRegionId,
    indent: &str,
) -> String {
    let lower = tag.to_ascii_lowercase();
    match lower.as_str() {
        "column" => emit_container(ctx, out, "awsl_column", children, indent),
        "row" => emit_container(ctx, out, "awsl_row", children, indent),
        "box" => emit_container(ctx, out, "awsl_box", children, indent),
        "list" => emit_container(ctx, out, "awsl_list", children, indent),
        "text" => {
            let text_expr = text_expr_or_blank(ctx, children);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_text({text_expr})").unwrap();
            var
        }
        "button" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let event = extract_event(ctx.module, attrs).unwrap_or_default();
            let callback = ctx.event_callback(&event);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_button({label_expr}, {callback})").unwrap();
            var
        }
        "item" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let event = extract_event(ctx.module, attrs).unwrap_or_default();
            let callback = ctx.event_callback(&event);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_item({label_expr}, {callback})").unwrap();
            var
        }
        "checkbox" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let binding = extract_binding_name(ctx.module, attrs).unwrap_or_default();
            let initial = flag_initial(ctx.bindings, &binding);
            let name_lit = encode_v_text_literal(&binding);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_checkbox({label_expr}, {name_lit}, {initial}, session)").unwrap();
            var
        }
        "radio" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let binding = extract_binding_name(ctx.module, attrs).unwrap_or_default();
            let option_val = extract_option_value(ctx.module, attrs).unwrap_or_default();
            let name_lit = encode_v_text_literal(&binding);
            let option_lit = encode_v_text_literal(&option_val);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_radio({label_expr}, {name_lit}, {option_lit}, session)").unwrap();
            var
        }
        "radiogroup" => emit_container(ctx, out, "awsl_column", children, indent),
        "flex" | "slot" => emit_container(ctx, out, "awsl_column", children, indent),
        _ => {
            let text_expr = text_expr_or_blank(ctx, children);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_text({text_expr})").unwrap();
            var
        }
    }
}

/// 生成容器 widget（column/row/box/list）：先构建子节点列表，再包装。
fn emit_container(ctx: &mut TerminalVCodegen<'_>, out: &mut String, ctor: &str, children: RenderRegionId, indent: &str) -> String {
    let list_var = format!("children_{}", ctx.var_counter);
    ctx.var_counter += 1;
    writeln!(out, "{indent}let {list_var}: ArrayList<Widget> = ArrayList::new(8)").unwrap();
    for &child_id in region_nodes(ctx.module, children) {
        let child_var = emit_node(ctx, out, child_id, indent);
        if !child_var.is_empty() {
            writeln!(out, "{indent}{list_var}.push({child_var})").unwrap();
        }
    }
    let var = ctx.next_var();
    writeln!(out, "{indent}let {var}: Widget = {ctor}({list_var})").unwrap();
    var
}

fn emit_fragment(ctx: &mut TerminalVCodegen<'_>, out: &mut String, children: RenderRegionId, indent: &str) -> String {
    let child_ids = region_nodes(ctx.module, children);
    if child_ids.len() == 1 {
        return emit_node(ctx, out, child_ids[0], indent);
    }
    emit_container(ctx, out, "awsl_column", children, indent)
}

fn emit_if(ctx: &mut TerminalVCodegen<'_>, out: &mut String, render_if: &RenderIfNode, indent: &str) -> String {
    let var = ctx.next_var();
    let cond = eval_cond_expr(strip_expr_braces(ctx.module.expr_source(render_if.condition)), ctx.bindings);
    writeln!(out, "{indent}let mut {var}: Widget = awsl_text(\"\")").unwrap();
    writeln!(out, "{indent}if {cond} {{").unwrap();
    {
        let inner = format!("{indent}    ");
        for &node_id in region_nodes(ctx.module, render_if.then_region) {
            let child_var = emit_node(ctx, out, node_id, &inner);
            if !child_var.is_empty() {
                writeln!(out, "{inner}{var} = {child_var}").unwrap();
                break;
            }
        }
    }
    writeln!(out, "{indent}}} else {{").unwrap();
    {
        let inner = format!("{indent}    ");
        for &node_id in region_nodes(ctx.module, render_if.else_region) {
            let child_var = emit_node(ctx, out, node_id, &inner);
            if !child_var.is_empty() {
                writeln!(out, "{inner}{var} = {child_var}").unwrap();
                break;
            }
        }
    }
    writeln!(out, "{indent}}}").unwrap();
    var
}

fn emit_loop(ctx: &mut TerminalVCodegen<'_>, out: &mut String, render_loop: &RenderLoopNode, indent: &str) -> String {
    let list_var = format!("loop_{}", ctx.var_counter);
    ctx.var_counter += 1;
    writeln!(out, "{indent}let {list_var}: ArrayList<Widget> = ArrayList::new(8)").unwrap();
    writeln!(out, "{indent}# loop: {} in {}", render_loop.item_var, ctx.module.expr_source(render_loop.items)).unwrap();
    let var = ctx.next_var();
    writeln!(out, "{indent}let {var}: Widget = awsl_list({list_var})").unwrap();
    var
}

fn emit_text(ctx: &mut TerminalVCodegen<'_>, out: &mut String, segments: &[RenderTextSegment], indent: &str) -> String {
    let expr = build_text_expr(ctx.module, segments, ctx.bindings);
    let var = ctx.next_var();
    writeln!(out, "{indent}let {var}: Widget = awsl_text({expr})").unwrap();
    var
}

/// 提取区域的全部文本段并求值为表达式；区域无内容时返回空串（调用方决定占位）。
fn extract_text_expr(ctx: &TerminalVCodegen<'_>, region: RenderRegionId) -> String {
    let module = ctx.module;
    let mut segments: Vec<RenderTextSegment> = Vec::new();
    for &node_id in region_nodes(module, region) {
        match module.node(node_id) {
            RenderNode::Text(text) => segments.extend(text.segments.clone()),
            RenderNode::Element(element) => {
                let sub = extract_text_expr(ctx, element.children);
                if !sub.is_empty() {
                    segments.push(RenderTextSegment::Static(sub));
                }
            }
            RenderNode::Component(component) => {
                let sub = extract_text_expr(ctx, component.children);
                if !sub.is_empty() {
                    segments.push(RenderTextSegment::Static(sub));
                }
            }
            RenderNode::Fragment(fragment) => {
                let sub = extract_text_expr(ctx, fragment.children);
                if !sub.is_empty() {
                    segments.push(RenderTextSegment::Static(sub));
                }
            }
            _ => {}
        }
    }
    if segments.is_empty() {
        return String::new();
    }
    build_text_expr(module, &segments, ctx.bindings)
}

/// 标签/文本表达式：区域无内容时落到空文本字面量，保证 `awsl_*` 调用实参完整。
fn text_expr_or_blank(ctx: &TerminalVCodegen<'_>, region: RenderRegionId) -> String {
    let expr = extract_text_expr(ctx, region);
    if expr.is_empty() { "\"\"".to_string() } else { expr }
}

fn build_text_expr(module: &RenderModule, segments: &[RenderTextSegment], bindings: &[ScriptBinding]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for segment in segments {
        match segment {
            RenderTextSegment::Static(text) => {
                if !text.is_empty() {
                    parts.push(encode_v_text_literal(text));
                }
            }
            RenderTextSegment::Expr(expr_id) => {
                parts.push(eval_text_expr(module.expr_source(*expr_id), bindings));
            }
        }
    }
    if parts.is_empty() {
        return "\"\"".to_string();
    }
    parts.join(" ++ ")
}

/// 文本位置的表达式求值：响应式绑定（布尔/选项/整数）读当前会话的类型化槽（经运行帧
/// 按显示名换取句柄，与控件构造共享同一登记），非响应式绑定内联其 init 表达式；其余
/// 表达式直接作为 V 代码求值并按 `Display` 合同投影为文本。
fn eval_text_expr(expr: &str, bindings: &[ScriptBinding]) -> String {
    eval_text_expr_guarded(expr, bindings, &mut Vec::new())
}

fn eval_text_expr_guarded(expr: &str, bindings: &[ScriptBinding], visiting: &mut Vec<String>) -> String {
    let trimmed = strip_expr_braces(expr);
    if trimmed.is_empty() {
        return "\"\"".to_string();
    }
    for binding in bindings {
        if binding.reactive && trimmed == binding.name {
            // `Utf8` 槽本身就是文本，`Bool`/整数走 `Display` 合同投影。
            return match binding.value_type {
                SignalValueType::Utf8 => terminal_slot_read(binding),
                SignalValueType::Bool | SignalValueType::I32 => display_of(&terminal_slot_read(binding)),
            };
        }
    }
    for binding in bindings {
        if !binding.reactive && !binding.init_expr.trim().is_empty() && trimmed == binding.name && !visiting.iter().any(|v| v == &binding.name)
        {
            visiting.push(binding.name.clone());
            let expanded = eval_text_expr_guarded(&binding.init_expr, bindings, visiting);
            visiting.pop();
            return expanded;
        }
    }
    let out = substitute_terminal_reads(trimmed, bindings);
    if out.starts_with('"') && out.ends_with('"') && out.len() >= 2 {
        return out;
    }
    display_of(&out)
}

/// 条件表达式求值：布尔绑定读类型化槽，非响应式绑定内联 init，其余保持原表达式
/// （未解析的名字由编译面失败闭合）。不存在"字符串真值"判定通道。
fn eval_cond_expr(expr: &str, bindings: &[ScriptBinding]) -> String {
    let trimmed = expr.trim();
    for binding in bindings {
        if binding.reactive && trimmed == binding.name {
            return terminal_slot_read(binding);
        }
    }
    for binding in bindings {
        if !binding.reactive && !binding.init_expr.trim().is_empty() && trimmed == binding.name {
            return eval_cond_expr(&binding.init_expr, bindings);
        }
    }
    substitute_terminal_reads(trimmed, bindings)
}

/// 复合表达式中的绑定替换：响应式绑定换成类型化槽读取，非响应式绑定内联 init。
fn substitute_terminal_reads(expr: &str, bindings: &[ScriptBinding]) -> String {
    let mut out = expr.to_string();
    for binding in bindings.iter().filter(|binding| binding.reactive) {
        out = replace_ident(&out, &binding.name, &format!("({})", terminal_slot_read(binding)));
    }
    for binding in bindings.iter().filter(|binding| !binding.reactive && !binding.init_expr.trim().is_empty()) {
        out = replace_ident(&out, &binding.name, &format!("({})", binding.init_expr.trim()));
    }
    out.replace("===", "==").replace("!==", "!=")
}

/// 响应式绑定的类型化槽读取表达式：布尔走 `FlagBinding` 槽，单选组走 `ChoiceBinding`
/// 槽，整数走 `IntBinding` 槽，句柄经当前运行帧按显示名换取（与 `awsl_checkbox` /
/// `awsl_radio` 同一登记身份；整数槽的补丁通道为 `asgard_terminal_patch_int`）。
fn terminal_slot_read(binding: &ScriptBinding) -> String {
    let name_lit = encode_v_text_literal(&binding.name);
    match binding.value_type {
        SignalValueType::Bool => {
            let initial = bool_init(&binding.init_expr);
            format!("session.flag(AsgardTuiBindings.flag(session, {name_lit}, {initial}))")
        }
        SignalValueType::Utf8 => {
            let initial = choice_init(&binding.init_expr);
            format!("session.choice(AsgardTuiBindings.choice(session, {name_lit}, {initial}))")
        }
        SignalValueType::I32 => {
            let initial = int_init(&binding.init_expr);
            format!("session.int(AsgardTuiBindings.int(session, {name_lit}, {initial}))")
        }
    }
}

fn display_of(expr: &str) -> String {
    format!("({expr}).display()")
}

/// 复选框初始值：脚本绑定声明的初始表达式，缺省时为 `false`。
fn flag_initial(bindings: &[ScriptBinding], name: &str) -> String {
    bindings.iter().find(|binding| binding.name == name).map(|binding| bool_init(&binding.init_expr)).unwrap_or_else(|| "false".to_string())
}

fn bool_init(init_expr: &str) -> String {
    let trimmed = init_expr.trim();
    if trimmed.is_empty() { "false".to_string() } else { trimmed.to_string() }
}

fn choice_init(init_expr: &str) -> String {
    let trimmed = init_expr.trim();
    if trimmed.is_empty() { "\"\"".to_string() } else { trimmed.to_string() }
}

/// 整数槽初值：脚本绑定声明的初始表达式（可为负字面量），缺省时为 `0`。
fn int_init(init_expr: &str) -> String {
    let trimmed = init_expr.trim();
    if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
}

fn extract_event(module: &RenderModule, attrs: &[RenderAttr]) -> Option<String> {
    for attr in attrs {
        if attr.is_event && (attr.name == "@click" || attr.name == "on:click") {
            return Some(attr_value_source(module, &attr.value));
        }
    }
    None
}

fn extract_binding_name(module: &RenderModule, attrs: &[RenderAttr]) -> Option<String> {
    for attr in attrs {
        if attr.name == ":binding" || attr.name == "binding" {
            return attr_value_string(module, &attr.value);
        }
    }
    None
}

fn extract_option_value(module: &RenderModule, attrs: &[RenderAttr]) -> Option<String> {
    for attr in attrs {
        if attr.name == ":option" || attr.name == "option" || attr.name == "value" {
            return attr_value_string(module, &attr.value);
        }
    }
    None
}

fn attr_value_string(module: &RenderModule, value: &RenderAttrValue) -> Option<String> {
    match value {
        RenderAttrValue::Static(text) => Some(text.clone()),
        RenderAttrValue::Expr(expr_id) => Some(module.expr_source(*expr_id).to_string()),
        RenderAttrValue::Template(segments) => Some(
            segments
                .iter()
                .map(|segment| match segment {
                    RenderTextSegment::Static(text) => text.clone(),
                    RenderTextSegment::Expr(expr_id) => module.expr_source(*expr_id).to_string(),
                })
                .collect(),
        ),
    }
}

/// 生成事件处理函数：函数值回调，签名由 `std.terminal` 控件行为字段固定为
/// `micro(mut TuiSession) -> unit`；不存在"原始桩 + 重挂载包装"的双函数通道。
fn emit_handlers(handler_names: &[String]) -> String {
    let mut out = String::new();
    for full_name in handler_names {
        let original = full_name.strip_prefix("awsl_call_").unwrap_or(full_name);
        writeln!(out, "# 事件处理函数 `{original}`：函数值回调，接收当前会话的可变引用。").unwrap();
        writeln!(out, "micro {full_name}(session: mut TuiSession) -> unit {{").unwrap();
        writeln!(
            out,
            "    # 用户逻辑：经 asgard_terminal_patch_flag / asgard_terminal_patch_choice / asgard_terminal_patch_int 按显示名更新类型化槽，"
        )
        .unwrap();
        writeln!(out, "    # 或 asgard_terminal_remount(session, awsl_build_*) 重建控件树后由事件循环继续。").unwrap();
        writeln!(out, "}}\n").unwrap();
    }
    out
}

/// 组件级空操作回调：无事件按钮/条目的函数值占位。
fn emit_noop(route: &str) -> String {
    let mut out = String::new();
    writeln!(out, "# 空操作回调：缺事件的可交互控件的函数值占位。").unwrap();
    writeln!(out, "micro awsl_noop_{route}(session: mut TuiSession) -> unit {{").unwrap();
    writeln!(out, "}}\n").unwrap();
    out
}

/// 主入口：控件树构造函数以函数值交给产品入口 `asgard_terminal_run`；
/// 会话由产品入口显式创建并逐参数传递，生成代码不自持会话、不读取任何全局运行时。
fn emit_main(route: &str) -> String {
    let mut out = String::new();
    writeln!(out, "# 主入口：把带 session 的控件树构造函数交给产品入口，会话由产品入口创建。").unwrap();
    writeln!(out, "micro awsl_main() -> i32 {{").unwrap();
    writeln!(out, "    return asgard_terminal_run(awsl_build_{route})").unwrap();
    writeln!(out, "}}").unwrap();
    out
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase()
}
