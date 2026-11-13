//! 合并 AWSL 降级结果为单一 WASM 编译单元。

use std::fmt::Write as _;

use nyar_types::compiler_input::CompilerSourceFile;

use crate::asgard::{
    awsl::{LoweredComponent, ScriptBinding, SignalValueType, prepare_components_for_generation},
    codegen::{
        mobile_prelude::MOBILE_HOST_PRELUDE, mp_prelude::MP_SIG_PRELUDE, reactive_prelude::REACTIVE_PRELUDE,
        terminal_v_render::render_terminal_component_v, ui_host_abi::resolve_sig_export, v_prelude::DOM_PRELUDE, v_render::render_component_v,
    },
    error::AsgardResult,
};

/// 生成单元标量载体的形态。
///
/// 生成代码直接用 `i32` 拼写并传字面量：字面量只有落在 `[primitive]` 身份上才能参与
/// 重载匹配，缺少载体时 `sig_set_i32(sv, 0)` 这类调用会以「无可行重载」失败。
/// 载体来源由生成组可见性唯一决定，生成器先判定形态再产出源码，不在文本上事后裁剪。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarBootstrap {
    /// 生成组显式依赖 std：std 的规范 `[primitive]` 声明已进入可见面，生成源码只经
    /// `using std.primitive;` 消费它，不重声明、不投影。
    StdVisible,
    /// 封闭生成单元（std 不可见）：载体作为独立源文件随生成组提交。
    ///
    /// 该文件的文件级 `namespace std.primitive;` 只约束它自身，生成代码文件留在根命名空间。
    ClosedUnit,
}

/// 生成单元消费 std 标量声明的显式导入（与既有 `using std.text;` 并列）。
const GENERATED_STD_PRIMITIVE_IMPORT: &str = "using std.primitive;\n\n";

/// 封闭生成单元的标量载体源文件身份。
const GENERATED_SCALARS_PATH: &str = "asgard/generated.scalars.v";

/// 封闭生成单元的标量载体文本。
///
/// 载体必须占据身份的规范位置（`namespace std.primitive;`）：封闭单元见不到 std 的
/// 规范声明，身份引用 `std::primitive::i32` 只能在自身可见面内自命中；根命名空间下的
/// 重声明无法证明同源，会被闭包身份来源门结构化拒绝。这里只声明数值标量 `i32`。
///
/// `bool` 不在这里声明：它是语言编码型内建（类型名解析时 `encoding_builtin_type` 优先于
/// 同名声明），本地声明只会成为同一身份的重复载体；移除必须与跨 surface 别名登记同批。
const GENERATED_SCALARS_SOURCE: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }
"#;

impl ScalarBootstrap {
    /// 生成代码文件开头的标量导入；封闭单元的载体在独立文件里，主文件不写。
    fn prelude(self) -> &'static str {
        match self {
            Self::StdVisible => GENERATED_STD_PRIMITIVE_IMPORT,
            Self::ClosedUnit => "",
        }
    }

    /// 随生成组提交的附加源文件。
    fn extra_sources(self) -> Vec<CompilerSourceFile> {
        match self {
            Self::StdVisible => Vec::new(),
            Self::ClosedUnit => {
                vec![CompilerSourceFile { path: GENERATED_SCALARS_PATH.into(), source: GENERATED_SCALARS_SOURCE.into() }]
            }
        }
    }
}

/// 生成单元：主源码与随生成组提交的附加源文件。
#[derive(Debug, Clone)]
pub struct GeneratedVSource {
    /// 生成代码文件（`asgard/generated.v`）文本。
    pub source: String,
    /// 生成组内的附加源文件（封闭单元的标量载体）；std 可见时为空。
    pub extra_sources: Vec<CompilerSourceFile>,
}

/// 生成单元显式导入的 std 合同。
///
/// 文本投影（`<expr>.display()`）依赖 `std.text` 的 `Display` 与 `format` 声明。
/// 生成源码是封闭编译单元，不继承依赖包的语义导出，也不赌隐式可见性：`display`
/// 必须经这条 `using` 解析到 `std.text` 的具体 `imply` 成员，否则调用点以成员解析
/// 失败闭合（诊断带接收者类型名），绝不回退字符串拼接或 `format` 兜底。
const GENERATED_STD_TEXT_IMPORT: &str = "using std.text;\n\n";

/// 从所有 AWSL 组件生成可编译的 V 源码（浏览器：DOM + reactive）。
///
/// 生成入口统一做“先校验、后降级”：ABI 错误在产出任何 V 之前失败，
/// 不允许漂到 V 编译期。`bootstrap` 决定标量载体形态，由调用方在构建前按生成组
/// 可见性判定一次。
pub fn build_awsl_wasm_source(components: &[LoweredComponent], bootstrap: ScalarBootstrap) -> AsgardResult<GeneratedVSource> {
    let prepared = prepare_components_for_generation(components)?;
    Ok(build_awsl_source_with_prelude(&prepared, DOM_PRELUDE, REACTIVE_PRELUDE, bootstrap))
}

/// 从所有 AWSL 组件生成移动端/桌面宿主 V 源码（原生 patch/event FFI）。
pub fn build_awsl_host_source(components: &[LoweredComponent], bootstrap: ScalarBootstrap) -> AsgardResult<GeneratedVSource> {
    let prepared = prepare_components_for_generation(components)?;
    Ok(build_awsl_source_with_prelude(&prepared, MOBILE_HOST_PRELUDE, REACTIVE_PRELUDE, bootstrap))
}

/// 终端生成段的命名空间与显式导入。
///
/// 生成代码是 `asgard.generated` 命名空间下的纯消费方：类型与句柄来自 `std.terminal`
/// （冻结会话外形），控件构造原语与运行入口来自 `asgard.terminal`（产品模块）。
/// host_contract 声明归 `std.terminal` 唯一所有，生成单元不复制、不携带任何标准库
/// 或产品运行时源码副本；两者必须由项目依赖闭包提供，缺失即编译面失败闭合。
const TERMINAL_GENERATED_NAMESPACE: &str = "namespace asgard.generated;\n\nusing std.terminal;\nusing asgard.terminal;\n\n";

/// 从所有 AWSL 组件生成终端 TUI 宿主 V 源码（冻结 session 外形的控件树构造）。
///
/// 终端路径消费 `std.terminal` 显式会话与 `asgard.terminal` 产品入口：生成代码只含
/// 带 session 参数的控件树构造函数、函数值事件回调与交给 `asgard_terminal_run` 的
/// 入口调用；不依赖 DOM / 信号系统，不内嵌运行时源码，也没有全局绑定存储通道。
pub fn build_awsl_terminal_source(components: &[LoweredComponent], bootstrap: ScalarBootstrap) -> AsgardResult<GeneratedVSource> {
    let components = prepare_components_for_generation(components)?;
    let mut out = bootstrap.prelude().to_string();
    out.push_str(GENERATED_STD_TEXT_IMPORT);
    out.push_str(TERMINAL_GENERATED_NAMESPACE);
    let mut generated = String::new();
    for component in &components {
        generated.push_str(&render_terminal_component_v(component)?);
        generated.push('\n');
    }
    push_host_callable_micros(&mut out, &generated);
    Ok(GeneratedVSource { source: out, extra_sources: bootstrap.extra_sources() })
}

/// 小程序逻辑 WASM：signal host + script / `awsl_call_*` / `awsl_sig_*`，无 DOM hydrate。
pub fn build_awsl_mp_source(components: &[LoweredComponent], bootstrap: ScalarBootstrap) -> AsgardResult<GeneratedVSource> {
    let components = prepare_components_for_generation(components)?;
    let mut out = bootstrap.prelude().to_string();
    out.push_str(GENERATED_STD_TEXT_IMPORT);
    out.push_str(MP_SIG_PRELUDE);
    out.push_str("micro awsl_key_none(): utf8 { return \"\" }\n\n");
    let mut script_section = String::new();
    let mut generated = String::new();
    for component in &components {
        emit_module_level_signal_slots(&mut out, component);
        emit_mp_component_init(&mut generated, component);
        // 脚本段是模块内部实现（信号重写后的 micro），永远不进宿主导出面。
        if !component.synthetic_v.trim().is_empty() {
            script_section.push_str(&component.synthetic_v);
            script_section.push('\n');
        }
        emit_component_call_wrappers(&mut generated, component);
        emit_mp_sig_value_exports(&mut generated, component);
    }
    out.push_str(&script_section);
    push_host_callable_micros(&mut out, &generated);
    Ok(GeneratedVSource { source: out, extra_sources: bootstrap.extra_sources() })
}

/// 生成源码的顶层 `micro` 是宿主 ABI 的一部分：JS / 原生宿主按名字直接调用，
/// 因此每条都必须显式 `[export]`。缺注解时编译虽然成功，但调用会静默落空。
///
/// 只处理生成段，不处理静态 prelude：prelude 里的辅助函数不是宿主调用表面。
/// 脚本段（`synthetic_v`）也不经过这里：用户脚本 micro 由 `awsl_call_*` 包装器
/// 转接，不是宿主入口；带自由名的脚本 micro 更不能被导出。
fn push_host_callable_micros(out: &mut String, generated: &str) {
    for line in generated.lines() {
        if line.starts_with("micro ") {
            out.push_str("[export]\n");
        }
        out.push_str(line);
        out.push('\n');
    }
}

fn emit_module_level_signal_slots(out: &mut String, component: &LoweredComponent) {
    let mut wrote = false;
    for binding in &component.script_bindings {
        // Memoized 绑定是每次 hydrate 重算的局部值（`rx_memo_*` 返回），不进模块作用域；
        // 其余绑定（响应式信号槽与非响应式 `let`）必须是模块级：expr / handler 导出
        // 函数在模块作用域引用它们，缺少模块级绑定时引用悬空、语义闭包失败闭合。
        if binding.kind == crate::asgard::awsl::BindingKind::Memoized {
            continue;
        }
        if !binding.reactive && binding.init_expr.trim().is_empty() {
            continue;
        }
        writeln!(out, "{}", binding.as_v_let_line()).unwrap();
        wrote = true;
    }
    if wrote {
        writeln!(out).unwrap();
    }
}

fn emit_mp_component_init(out: &mut String, component: &LoweredComponent) {
    let route = sanitize_route(&component.route_name);
    writeln!(out, "micro awsl_mp_init_{route}() {{").unwrap();
    for binding in &component.script_bindings {
        if !binding.reactive {
            continue;
        }
        let reset = match binding.value_type {
            SignalValueType::I32 => format!("sig_set_i32({}, {});", binding.sig_var, binding.init_expr),
            SignalValueType::Utf8 => format!("sig_set_utf8({}, {});", binding.sig_var, binding.init_expr),
            SignalValueType::Bool => format!("sig_set_bool({}, {});", binding.sig_var, binding.init_expr),
        };
        writeln!(out, "    {reset}").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

fn build_awsl_source_with_prelude(
    components: &[LoweredComponent],
    dom_or_mobile: &str,
    reactive: &str,
    bootstrap: ScalarBootstrap,
) -> GeneratedVSource {
    // 规范命名空间必须位于所有 `using` 与声明之前。
    let mut out = bootstrap.prelude().to_string();
    out.push_str(GENERATED_STD_TEXT_IMPORT);
    out.push_str(dom_or_mobile);
    out.push_str(reactive);
    out.push_str("micro awsl_key_none(): utf8 { return \"\" }\n\n");
    let mut script_section = String::new();
    let mut generated = String::new();
    for component in components {
        // 浏览器路径禁止模块级 `let`（HIR 静默丢弃模块级绑定）：响应式状态全部
        // 经宿主按名信号表（`sig_named`）寻址，非响应式绑定在使用点内联展开。
        // 脚本段单独落盘，避免被 `push_host_callable_micros` 当成宿主入口导出。
        if !component.synthetic_v.trim().is_empty() {
            script_section.push_str(&component.synthetic_v);
            script_section.push('\n');
        }
        emit_component_call_wrappers(&mut generated, component);
        emit_component_sig_exports(&mut generated, component);
        let script_body = script_lets_prelude(component);
        generated.push_str(&render_component_v(&component.route_name, &component.script_bindings, &script_body, &component.render_ir));
        generated.push('\n');
    }
    out.push_str(&script_section);
    push_host_callable_micros(&mut out, &generated);
    GeneratedVSource { source: out, extra_sources: bootstrap.extra_sources() }
}

fn emit_component_call_wrappers(out: &mut String, component: &LoweredComponent) {
    for line in component.synthetic_v.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("micro ") {
            if let Some(micro_name) = rest.split(|c: char| c.is_whitespace() || c == '(').next() {
                if !micro_name.is_empty() {
                    let Some((params, forwarded)) = mirror_micro_parameters(rest)
                    else {
                        continue;
                    };
                    writeln!(out, "micro awsl_call_{micro_name}({params}) {{\n    {micro_name}({forwarded})\n}}\n").unwrap();
                }
            }
        }
    }
    let _ = component.route_name.as_str();
}

/// 提取 `micro` 头部形参文本并按名生成转发实参。
///
/// 事件生成侧按 `awsl_call_{handler}(arg)` 调用包装器，包装器必须镜像原 micro
/// 的形参并逐个转发，否则包装体内会退化成 0 实参调用。形参无法解析时返回
/// `None`：调用方跳过包装，让缺失的 `awsl_call_*` 在调用点硬失败，禁止生成
/// 一个静默丢参的包装。
fn mirror_micro_parameters(rest: &str) -> Option<(&str, String)> {
    let open = rest.find('(')?;
    // 形参括号必须出现在函数体之前：函数体里的调用括号不是签名。
    if let Some(brace) = rest.find('{') {
        if brace < open {
            return None;
        }
    }
    let mut depth = 0usize;
    let mut close = None;
    for (offset, ch) in rest[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    close = Some(open + offset);
                    break;
                }
            }
            _ => {}
        }
    }
    let params = rest[open + 1..close?].trim();
    if params.is_empty() {
        return Some((params, String::new()));
    }
    let mut forwarded = Vec::new();
    for part in split_top_level_commas(params) {
        let part = part.trim();
        let name = part.split(':').next().unwrap_or("").trim();
        let name = name.strip_prefix("mut ").unwrap_or(name).trim();
        if name.is_empty() {
            return None;
        }
        forwarded.push(name.to_string());
    }
    Some((params, forwarded.join(", ")))
}

/// 按顶层逗号切分形参，括号/方括号/花括号/尖括号内的逗号不参与切分。
fn split_top_level_commas(params: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in params.char_indices() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&params[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&params[start..]);
    parts
}

fn emit_component_sig_exports(out: &mut String, component: &LoweredComponent) {
    for binding in &component.script_bindings {
        if !binding.reactive {
            continue;
        }
        let export = resolve_sig_export(&component.route_name, &binding.name);
        writeln!(out, "micro {export}(): i32 {{ return sig_named(\"{}\") }}", binding.sig_var).unwrap();
    }
}

/// 小程序：`awsl_sig_*` 返回当前值（供 setData 同步），不是 signal id。
fn emit_mp_sig_value_exports(out: &mut String, component: &LoweredComponent) {
    for binding in &component.script_bindings {
        if !binding.reactive {
            continue;
        }
        let export = resolve_sig_export(&component.route_name, &binding.name);
        match binding.value_type {
            SignalValueType::I32 => {
                writeln!(out, "micro {export}(): i32 {{ return sig_get_i32({}) }}", binding.sig_var).unwrap();
            }
            SignalValueType::Utf8 => {
                writeln!(out, "micro {export}(): utf8 {{ return sig_get_utf8({}) }}", binding.sig_var).unwrap();
            }
            SignalValueType::Bool => {
                writeln!(out, "micro {export}(): bool {{ return sig_get_bool({}) }}", binding.sig_var).unwrap();
            }
        }
    }
}

fn sanitize_route(name: &str) -> String {
    name.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_ascii_lowercase()
}

/// hydrate 函数体的信号初始化行。
///
/// 响应式绑定：`let {sig_var} = sig_named(key)` 取宿主按名句柄，再 `sig_set_*` 写入初值
/// （父组件随后经 `awsl_sig_*` + `rx_bind_prop_*` 覆盖 Property 默认值）。
/// 非响应式绑定不产生任何行——引用点已内联展开。
fn script_lets_prelude(component: &LoweredComponent) -> String {
    component
        .script_bindings
        .iter()
        .filter(|binding| binding.reactive)
        .map(|binding| {
            let setter = match binding.value_type {
                SignalValueType::I32 => "sig_set_i32",
                SignalValueType::Utf8 => "sig_set_utf8",
                SignalValueType::Bool => "sig_set_bool",
            };
            let sv = binding.sig_var.as_str();
            format!("let {sv} = sig_named(\"{sv}\")\n{setter}({sv}, {})", binding.init_expr)
        })
        .collect::<Vec<_>>()
        .join("\n    ")
}
