//! RenderIR → 终端 V 控件树构造函数（带 session 参数，产品 `awsl_*` 原语）。
//!
//! 终端路径消费冻结的 `std.terminal` 会话外形：控件树由产品模块 `asgard.terminal`
//! 的 `awsl_*` 原语构造；交互状态（布尔、单选组）以显示名经当前运行帧换取类型化槽
//! 句柄（`FlagBinding` / `ChoiceBinding`）；事件行为是函数值
//! `micro(mut TuiSession) -> unit`，构造控件时直接传入；纯展示文本在构造期直接求值
//! 写入 `Text` 控件。不存在绑定字符串登记、按名分派或全局运行时读写通道。
//!
//! 生成器只输出真实语义，禁止「外观成功、行为缺失」：
//!
//! - 事件处理器从 `<script>` 的正式脚本事实翻译：对响应式绑定的赋值生成经运行帧
//!   句柄的类型化槽补丁（`session.patch_*`），随后重挂载刷新展示文本；找不到处理
//!   函数、空处理器体、无法翻译的语句一律显式报错拒绝生成。
//! - 循环生成遍历全部元素的真实迭代（`loop item in items`）；循环体为空、循环源
//!   引用标量类型化槽或循环变量遮蔽脚本绑定时显式报错拒绝生成。
//! - 条件分支保留并生成区域内全部节点：单节点直接赋值，多节点包进容器。

use std::fmt::Write as _;

use crate::asgard::awsl::{
    LoweredComponent, RenderAttr, RenderAttrValue, RenderIfNode, RenderLoopNode, RenderModule, RenderNode, RenderNodeId, RenderRegionId,
    RenderTextSegment, ScriptBinding, SignalValueType,
    render_ir::{attr_value_source, region_nodes},
};
use crate::asgard::error::{AsgardError, AsgardResult};

use super::v_render::util::{encode_v_text_literal, is_simple_identifier, replace_ident, strip_expr_braces};

/// 将组件 RenderIR 生成为终端 V 源码（session 控件树构造 + 事件处理函数 + main）。
///
/// 任何无法生成真实语义的构造（空处理器、无法接线的循环）都以错误拒绝，
/// 不产出伪成功源码。
pub fn render_terminal_component_v(component: &LoweredComponent) -> AsgardResult<String> {
    let route = sanitize(&component.route_name);
    let mut out = String::new();
    let mut ctx = TerminalVCodegen {
        route: &route,
        module: &component.render_ir,
        bindings: &component.script_bindings,
        script_source: component.script_source.as_deref(),
        var_counter: 0,
        handlers: Vec::new(),
        needs_noop: false,
    };
    out.push_str(&emit_build(&mut ctx)?);
    out.push_str(&emit_handlers(&ctx.handlers));
    if ctx.needs_noop {
        out.push_str(&emit_noop(&route));
    }
    out.push_str(&emit_main(&route));
    Ok(out)
}

struct TerminalVCodegen<'a> {
    route: &'a str,
    module: &'a RenderModule,
    bindings: &'a [ScriptBinding],
    /// `<script>` 块原文：事件处理器的正式脚本事实来源。
    script_source: Option<&'a str>,
    var_counter: u32,
    /// 已翻译的事件处理函数：`awsl_call_*` 全名 → 完整函数源码文本。
    handlers: Vec<(String, String)>,
    needs_noop: bool,
}

impl<'a> TerminalVCodegen<'a> {
    fn next_var(&mut self) -> String {
        let name = format!("w{}", self.var_counter);
        self.var_counter += 1;
        name
    }

    fn reject_handler(&self, name: &str, reason: &str) -> AsgardError {
        AsgardError::msg(report!("组件 `{}` 的事件处理器 `{name}` 无法生成真实语义：{reason}", self.route))
    }

    /// 事件回调函数值：有 `@click` 时从脚本事实翻译处理函数，缺事件时落到组件级
    /// 空操作回调；控件行为字段是函数值类型，不存在空字符串占位通道。
    /// 翻译失败（处理器缺失、空体、无法接线）显式报错，禁止输出注释桩。
    fn event_callback(&mut self, event: &str) -> AsgardResult<String> {
        if event.is_empty() {
            self.needs_noop = true;
            return Ok(format!("awsl_noop_{}", self.route));
        }
        let name = normalize_handler(event)
            .ok_or_else(|| self.reject_handler(event, "处理器写法不是零参函数引用，无法接线到固定 `micro(mut TuiSession) -> unit` 回调"))?;
        let full = format!("awsl_call_{name}");
        if !self.handlers.iter().any(|(existing, _)| existing == &full) {
            let text = self.translate_handler(&name)?;
            self.handlers.push((full.clone(), text));
        }
        Ok(full)
    }

    /// 从 `<script>` 的正式脚本事实翻译处理器体：对响应式绑定的赋值逐条生成
    /// 经运行帧句柄的类型化槽补丁，随后重挂载刷新展示文本。
    fn translate_handler(&self, name: &str) -> AsgardResult<String> {
        let script = self
            .script_source
            .ok_or_else(|| self.reject_handler(name, "组件没有 `<script>` 段，处理器体没有正式脚本事实可翻译"))?;
        let (params, body) = find_script_micro(script, name)
            .ok_or_else(|| self.reject_handler(name, "`<script>` 中不存在同名 `micro` 定义，处理器没有可翻译的函数体"))?;
        if !params.is_empty() {
            return Err(self.reject_handler(name, "处理函数声明了形参，事件回调签名固定为 `micro(mut TuiSession) -> unit`，无法接线"));
        }
        let mut patches: Vec<(&ScriptBinding, String)> = Vec::new();
        for line in body.lines() {
            let statement = line.trim().trim_end_matches(';').trim();
            if statement.is_empty() || statement.starts_with('#') || statement.starts_with('⍝') {
                continue;
            }
            let Some((target, rhs)) = split_assignment(statement) else {
                return Err(self.reject_handler(name, &format!("语句 `{statement}` 不是对绑定的赋值，终端语义无法翻译")));
            };
            let Some(binding) = self.bindings.iter().find(|binding| binding.name == target) else {
                return Err(self.reject_handler(name, &format!("赋值目标 `{target}` 不是脚本绑定，没有类型化槽通道")));
            };
            if !binding.reactive {
                return Err(self.reject_handler(name, &format!("赋值目标 `{target}` 不是响应式绑定（`let mut`），没有会话槽可写")));
            }
            patches.push((binding, rhs.to_string()));
        }
        if patches.is_empty() {
            return Err(self.reject_handler(name, "处理器体为空，点击不会产生任何状态变化"));
        }
        // 需要句柄的绑定：补丁目标 + 任一赋值右值引用到的响应式绑定。
        let mut slots: Vec<&ScriptBinding> = Vec::new();
        for (target, rhs) in &patches {
            for binding in self.bindings.iter().filter(|binding| binding.reactive) {
                if (binding.name == target.name || expr_mentions(rhs, &binding.name)) && !slots.iter().any(|slot| slot.name == binding.name) {
                    slots.push(binding);
                }
            }
        }
        let mut out = String::new();
        writeln!(out, "# 事件处理函数 `{name}`：真实状态变更——经运行帧句柄写类型化槽，随后重挂载刷新展示文本。").unwrap();
        writeln!(out, "micro awsl_call_{name}(session: mut TuiSession) -> unit {{").unwrap();
        for binding in &slots {
            let (method, handle_type, initial) = slot_registration(binding);
            writeln!(out, "    let {}: {handle_type} = AsgardTuiBindings.{method}(session, {}, {initial})", slot_var(binding), encode_v_text_literal(&binding.name)).unwrap();
        }
        for (binding, rhs) in &patches {
            let value = substitute_handler_reads(rhs, self.bindings, &slots);
            writeln!(out, "    session.{}({}, {value})", patch_method(binding.value_type), slot_var(binding)).unwrap();
        }
        writeln!(out, "    asgard_terminal_remount(session, awsl_build_{})", self.route).unwrap();
        writeln!(out, "}}\n").unwrap();
        Ok(out)
    }
}

/// 归一化 `@click` 处理器写法：接受 `name`、`name()` 与 `() => name()` 形态，
/// 返回零参处理函数名；其余写法（带实参调用、任意表达式）无法接线，返回 `None`。
fn normalize_handler(event: &str) -> Option<String> {
    let mut handler = event.trim().trim_matches('"').trim_matches('\'').trim();
    if let Some(idx) = handler.find("=>") {
        let left = handler[..idx].trim();
        if left == "()" || left == "( )" {
            handler = handler[idx + 2..].trim();
        }
    }
    if is_simple_identifier(handler) {
        return Some(handler.to_string());
    }
    if handler.ends_with("()") {
        let fn_name = handler[..handler.len() - 2].trim();
        if is_simple_identifier(fn_name) {
            return Some(fn_name.to_string());
        }
    }
    None
}

/// 在 `<script>` 原文中定位 `micro <name>` 定义，返回形参文本与花括号体。
fn find_script_micro(script: &str, name: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = script.lines().collect();
    let mut index = 0usize;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if let Some(rest) = trimmed.strip_prefix("micro ").map(str::trim_start) {
            let end = rest.find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')).unwrap_or(rest.len());
            if &rest[..end] == name {
                let open = rest.find('(')?;
                let close = rest[open..].find(')')?;
                let params = rest[open + 1..open + close].trim().to_string();
                let mut depth = 0i32;
                let mut started = false;
                let mut block = String::new();
                let mut cursor = index;
                while cursor < lines.len() {
                    for ch in lines[cursor].chars() {
                        match ch {
                            '{' => {
                                depth += 1;
                                started = true;
                            }
                            '}' => depth -= 1,
                            _ => {}
                        }
                    }
                    if started {
                        block.push_str(lines[cursor]);
                        block.push('\n');
                    }
                    if started && depth == 0 {
                        break;
                    }
                    cursor += 1;
                }
                let Some(first) = block.find('{') else {
                    return Some((params, String::new()));
                };
                let Some(last) = block.rfind('}') else {
                    return Some((params, String::new()));
                };
                if last <= first {
                    return Some((params, String::new()));
                }
                return Some((params, block[first + 1..last].to_string()));
            }
        }
        index += 1;
    }
    None
}

/// 拆分赋值语句 `ident = rhs`；`==`/`!=`/`<=`/`>=` 比较与其它语句返回 `None`。
fn split_assignment(statement: &str) -> Option<(&str, &str)> {
    let bytes = statement.as_bytes();
    if bytes.is_empty() || !(bytes[0].is_ascii_alphabetic() || bytes[0] == b'_') {
        return None;
    }
    let mut i = 0usize;
    while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    let name = &statement[..i];
    let mut j = i;
    while j < bytes.len() && matches!(bytes[j], b' ' | b'\t') {
        j += 1;
    }
    if j >= bytes.len() || bytes[j] != b'=' || bytes.get(j + 1) == Some(&b'=') {
        return None;
    }
    Some((name, statement[j + 1..].trim()))
}

/// 表达式是否按标识符边界引用了 `name`（与 `replace_ident` 同一边界规则）。
fn expr_mentions(expr: &str, name: &str) -> bool {
    let chars: Vec<char> = expr.chars().collect();
    let name_chars: Vec<char> = name.chars().collect();
    let is_ident_char = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let mut i = 0usize;
    while i + name_chars.len() <= chars.len() {
        if chars[i..i + name_chars.len()] == name_chars[..] {
            let before_ok = i == 0 || (chars[i - 1] != '.' && !is_ident_char(chars[i - 1]));
            let after = i + name_chars.len();
            let after_ok = after >= chars.len() || !is_ident_char(chars[after]);
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// 类型化槽登记通道：运行帧方法名、句柄类型与登记初值（与控件构造同一事实源）。
fn slot_registration(binding: &ScriptBinding) -> (&'static str, &'static str, String) {
    match binding.value_type {
        SignalValueType::Bool => ("flag", "FlagBinding", bool_init(&binding.init_expr)),
        SignalValueType::Utf8 => ("choice", "ChoiceBinding", choice_init(&binding.init_expr)),
        SignalValueType::I32 => ("int", "IntBinding", int_init(&binding.init_expr)),
    }
}

fn slot_var(binding: &ScriptBinding) -> String {
    format!("slot_{}", sanitize(&binding.name))
}

fn read_method(value_type: SignalValueType) -> &'static str {
    match value_type {
        SignalValueType::Bool => "flag",
        SignalValueType::Utf8 => "choice",
        SignalValueType::I32 => "int",
    }
}

fn patch_method(value_type: SignalValueType) -> &'static str {
    match value_type {
        SignalValueType::Bool => "patch_flag",
        SignalValueType::Utf8 => "patch_choice",
        SignalValueType::I32 => "patch_int",
    }
}

/// 处理器右值中的绑定读取替换：已登记句柄的响应式绑定走 `session.{flag|choice|int}`
/// 句柄读取；未登记的响应式绑定经运行帧换取句柄后读取；非响应式绑定内联 init 表达式。
fn substitute_handler_reads(expr: &str, bindings: &[ScriptBinding], slots: &[&ScriptBinding]) -> String {
    let mut out = expr.to_string();
    for binding in bindings.iter().filter(|binding| binding.reactive) {
        let read = match slots.iter().find(|slot| slot.name == binding.name) {
            Some(slot) => format!("session.{}({})", read_method(binding.value_type), slot_var(slot)),
            None => terminal_slot_read(binding),
        };
        out = replace_ident(&out, &binding.name, &format!("({read})"));
    }
    for binding in bindings.iter().filter(|binding| !binding.reactive && !binding.init_expr.trim().is_empty()) {
        out = replace_ident(&out, &binding.name, &format!("({})", binding.init_expr.trim()));
    }
    out.replace("===", "==").replace("!==", "!=")
}

/// 生成 `awsl_build_<route>(session)`：带 session 的控件树构造。
/// 交互控件在这里经当前运行帧换取类型化槽句柄，纯展示文本直接求值。
fn emit_build(ctx: &mut TerminalVCodegen<'_>) -> AsgardResult<String> {
    let mut out = String::new();
    writeln!(out, "# 控件树构造：接收 session；交互状态经当前运行帧换取类型化槽句柄，展示文本直接求值。").unwrap();
    writeln!(out, "micro awsl_build_{}(session: mut TuiSession) -> ArrayList<Widget> {{", ctx.route).unwrap();
    writeln!(out, "    let roots: ArrayList<Widget> = ArrayList::new(16)").unwrap();
    for &root_id in &ctx.module.roots {
        let var = emit_node(ctx, &mut out, root_id, "    ")?;
        if !var.is_empty() {
            writeln!(out, "    roots.push({var})").unwrap();
        }
    }
    writeln!(out, "    return roots").unwrap();
    writeln!(out, "}}\n").unwrap();
    Ok(out)
}

/// 生成单个节点，返回变量名（空字符串表示不需要 push）。
fn emit_node(ctx: &mut TerminalVCodegen<'_>, out: &mut String, node_id: RenderNodeId, indent: &str) -> AsgardResult<String> {
    match ctx.module.node(node_id) {
        RenderNode::Element(element) => emit_tag(ctx, out, &element.tag, &element.attrs, element.children, indent),
        RenderNode::Component(component) => emit_tag(ctx, out, &component.tag, &component.attrs, component.children, indent),
        RenderNode::Text(text) => Ok(emit_text(ctx, out, &text.segments, indent)),
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
) -> AsgardResult<String> {
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
            Ok(var)
        }
        "button" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let event = extract_event(ctx.module, attrs).unwrap_or_default();
            let callback = ctx.event_callback(&event)?;
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_button({label_expr}, {callback})").unwrap();
            Ok(var)
        }
        "item" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let event = extract_event(ctx.module, attrs).unwrap_or_default();
            let callback = ctx.event_callback(&event)?;
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_item({label_expr}, {callback})").unwrap();
            Ok(var)
        }
        "checkbox" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let binding = extract_binding_name(ctx.module, attrs).unwrap_or_default();
            let initial = flag_initial(ctx.bindings, &binding);
            let name_lit = encode_v_text_literal(&binding);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_checkbox({label_expr}, {name_lit}, {initial}, session)").unwrap();
            Ok(var)
        }
        "radio" => {
            let label_expr = text_expr_or_blank(ctx, children);
            let binding = extract_binding_name(ctx.module, attrs).unwrap_or_default();
            let option_val = extract_option_value(ctx.module, attrs).unwrap_or_default();
            let name_lit = encode_v_text_literal(&binding);
            let option_lit = encode_v_text_literal(&option_val);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_radio({label_expr}, {name_lit}, {option_lit}, session)").unwrap();
            Ok(var)
        }
        "radiogroup" => emit_container(ctx, out, "awsl_column", children, indent),
        "flex" | "slot" => emit_container(ctx, out, "awsl_column", children, indent),
        _ => {
            let text_expr = text_expr_or_blank(ctx, children);
            let var = ctx.next_var();
            writeln!(out, "{indent}let {var}: Widget = awsl_text({text_expr})").unwrap();
            Ok(var)
        }
    }
}

/// 生成容器 widget（column/row/box/list）：先构建子节点列表，再包装。
fn emit_container(ctx: &mut TerminalVCodegen<'_>, out: &mut String, ctor: &str, children: RenderRegionId, indent: &str) -> AsgardResult<String> {
    let list_var = format!("children_{}", ctx.var_counter);
    ctx.var_counter += 1;
    writeln!(out, "{indent}let {list_var}: ArrayList<Widget> = ArrayList::new(8)").unwrap();
    for &child_id in region_nodes(ctx.module, children) {
        let child_var = emit_node(ctx, out, child_id, indent)?;
        if !child_var.is_empty() {
            writeln!(out, "{indent}{list_var}.push({child_var})").unwrap();
        }
    }
    let var = ctx.next_var();
    writeln!(out, "{indent}let {var}: Widget = {ctor}({list_var})").unwrap();
    Ok(var)
}

fn emit_fragment(ctx: &mut TerminalVCodegen<'_>, out: &mut String, children: RenderRegionId, indent: &str) -> AsgardResult<String> {
    let child_ids = region_nodes(ctx.module, children);
    if child_ids.len() == 1 {
        return emit_node(ctx, out, child_ids[0], indent);
    }
    emit_container(ctx, out, "awsl_column", children, indent)
}

/// 生成条件节点：两个分支都保留并生成区域内全部节点，不再只取首个。
fn emit_if(ctx: &mut TerminalVCodegen<'_>, out: &mut String, render_if: &RenderIfNode, indent: &str) -> AsgardResult<String> {
    let var = ctx.next_var();
    let cond = eval_cond_expr(strip_expr_braces(ctx.module.expr_source(render_if.condition)), ctx.bindings);
    writeln!(out, "{indent}let mut {var}: Widget = awsl_text(\"\")").unwrap();
    writeln!(out, "{indent}if {cond} {{").unwrap();
    emit_branch(ctx, out, render_if.then_region, &format!("{indent}    "), &var)?;
    writeln!(out, "{indent}}} else {{").unwrap();
    emit_branch(ctx, out, render_if.else_region, &format!("{indent}    "), &var)?;
    writeln!(out, "{indent}}}").unwrap();
    Ok(var)
}

/// 生成条件分支的全部节点：单节点直接赋值；多节点先构建子列表再包进
/// `awsl_column`（与片段多子节点同一包裹事实）；空分支保持占位文本。
fn emit_branch(ctx: &mut TerminalVCodegen<'_>, out: &mut String, region: RenderRegionId, inner: &str, target: &str) -> AsgardResult<()> {
    let mut vars: Vec<String> = Vec::new();
    for &node_id in region_nodes(ctx.module, region) {
        let child_var = emit_node(ctx, out, node_id, inner)?;
        if !child_var.is_empty() {
            vars.push(child_var);
        }
    }
    match vars.len() {
        0 => {}
        1 => writeln!(out, "{inner}{target} = {}", vars[0]).unwrap(),
        _ => {
            let list_var = format!("children_{}", ctx.var_counter);
            ctx.var_counter += 1;
            writeln!(out, "{inner}let {list_var}: ArrayList<Widget> = ArrayList::new({})", vars.len()).unwrap();
            for child_var in &vars {
                writeln!(out, "{inner}{list_var}.push({child_var})").unwrap();
            }
            writeln!(out, "{inner}{target} = awsl_column({list_var})").unwrap();
        }
    }
    Ok(())
}

/// 生成循环节点：真实迭代遍历全部元素，每个条目重建循环体控件并收集进列表。
/// 空循环体、标量类型化槽循环源与遮蔽脚本绑定的循环变量都无法生成真实语义，
/// 显式报错拒绝。
fn emit_loop(ctx: &mut TerminalVCodegen<'_>, out: &mut String, render_loop: &RenderLoopNode, indent: &str) -> AsgardResult<String> {
    if region_nodes(ctx.module, render_loop.body_region).is_empty() {
        return Err(AsgardError::msg(report!(
            "组件 `{}` 的循环（span {}..{}）循环体为空：无法生成遍历全部元素的真实迭代，拒绝生成",
            ctx.route,
            render_loop.span.start,
            render_loop.span.end
        )));
    }
    let items = strip_expr_braces(ctx.module.expr_source(render_loop.items));
    for binding in ctx.bindings.iter().filter(|binding| binding.reactive) {
        if expr_mentions(items, &binding.name) {
            return Err(AsgardError::msg(report!(
                "组件 `{}` 的循环源 `{items}` 引用响应式绑定 `{}`：类型化槽是标量值，没有可迭代的正式事实，拒绝生成",
                ctx.route,
                binding.name
            )));
        }
    }
    if ctx.bindings.iter().any(|binding| binding.name == render_loop.item_var) {
        return Err(AsgardError::msg(report!(
            "组件 `{}` 的循环变量 `{}` 遮蔽脚本绑定：迭代读取无法接线，拒绝生成",
            ctx.route,
            render_loop.item_var
        )));
    }
    let substituted = substitute_terminal_reads(items, ctx.bindings);
    let list_var = format!("loop_{}", ctx.var_counter);
    ctx.var_counter += 1;
    writeln!(out, "{indent}let {list_var}: ArrayList<Widget> = ArrayList::new(8)").unwrap();
    writeln!(out, "{indent}loop {} in {substituted} {{", render_loop.item_var).unwrap();
    let inner = format!("{indent}    ");
    for &node_id in region_nodes(ctx.module, render_loop.body_region) {
        let child_var = emit_node(ctx, out, node_id, &inner)?;
        if !child_var.is_empty() {
            writeln!(out, "{inner}{list_var}.push({child_var})").unwrap();
        }
    }
    writeln!(out, "{indent}}}").unwrap();
    let var = ctx.next_var();
    writeln!(out, "{indent}let {var}: Widget = awsl_list({list_var})").unwrap();
    Ok(var)
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

/// 汇总已翻译的事件处理函数；每个函数体都是从脚本事实生成的真实状态变更，
/// 不存在注释桩。签名由 `std.terminal` 控件行为字段固定为
/// `micro(mut TuiSession) -> unit`；不存在"原始桩 + 重挂载包装"的双函数通道。
fn emit_handlers(handlers: &[(String, String)]) -> String {
    let mut out = String::new();
    for (_, text) in handlers {
        out.push_str(text);
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
