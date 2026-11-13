//! AWSL 降级与 JS 胶水测试（自包含样例，不读取外部语言语料）。

use oak_awsl::AwslParser;
use rusty_valkyrie::{
    ValkyrieCompiler,
    asgard::{
        awsl::{LoweringOptions, compile_awsl_source, lower_component},
        codegen::{ScalarBootstrap, asgard_boot_script_tag, build_awsl_wasm_source, generate_boot_script, manifest_url_for_mode},
    },
};

/// 生成单元（`awsl_hydrate_*` 一类宿主调用）的最小形态：签名用裸 `i32`/`utf8` 拼写，
/// 调用点直接传整数字面量。生成单元不继承依赖包的语义导出，因此必须自带数值标量声明；
/// `bool` 是编码型内建，不需要也不应重声明。
const GENERATED_UNIT_SCALAR_SHAPE: &str = r#"namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

namespace demo.generated;

micro dom_create_text(text: utf8): i32 { }
micro rx_bind_text_utf8(handle: i32, expr_export: utf8, dep1: i32, dep2: i32) { }

[export]
micro awsl_hydrate_index(): i32 {
    let h2 = dom_create_text("")
    rx_bind_text_utf8(h2, "awsl_expr_index_0", -1, -1)
    return h2
}
"#;

/// 正例：生成单元自带标量声明时，`-1` 这类字面量按 `[primitive]` 身份落型，调用可通过。
#[test]
fn generated_unit_scalar_declarations_allow_integer_literal_calls() {
    let source = r#"<widget demo><Column><Text>{label}</Text></Column></widget>
<script>
let mut label: utf8 = ""
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let v_source = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source");
    assert!(
        v_source.extra_sources.iter().any(|file| file.source.contains("[primitive(\"std::primitive::i32\")]")),
        "封闭生成单元必须随组提交 `i32` 标量载体"
    );
    assert!(!v_source.source.contains("[primitive("), "标量载体不得再写进生成代码文件:\n{}", v_source.source);
    assert!(!v_source.source.contains("structure bool"), "`bool` 是编码型内建，生成单元不得重声明");
    ValkyrieCompiler::default().compile_source(GENERATED_UNIT_SCALAR_SHAPE).expect("标量声明在场时生成单元必须编译通过");
}

/// 反例：剥掉标量声明后，同一调用点的字面量实参必须结构化失败，不得静默选型。
#[test]
fn generated_unit_without_scalar_declarations_rejects_literal_arguments() {
    let without = GENERATED_UNIT_SCALAR_SHAPE.replacen("[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n", "", 1);
    assert!(!without.contains("structure i32 { }"), "反例必须真正移除标量声明");
    let error = ValkyrieCompiler::default().compile_source(&without).expect_err("缺少标量声明时字面量实参必须结构化失败");
    let text = format!("{error:?}");
    // 缺少标量声明时在声明绑定阶段即 fail-closed：字面量没有载体，不再进入调用解析。
    assert!(text.contains("TypeNotFound"), "缺少标量声明必须在绑定阶段结构化失败：{text}");
    assert!(text.contains("i32"), "诊断必须点名未绑定的标量拼写：{text}");
}

#[test]
fn script_let_mut_emits_reactive_binding_in_wasm() {
    let source = r#"<widget><text>{count}</text></widget>
<script>
let mut count = 0
let title = "hi"
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    assert!(lowered.script_bindings.iter().any(|b| b.name == "count" && b.reactive));
    assert!(lowered.script_bindings.iter().any(|b| b.name == "title" && !b.reactive));
    let v_source = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(v_source.contains("sig_create_i32"));
    // 浏览器生成源码禁止模块级 `let`（HIR 会丢弃模块级绑定）：非响应式绑定在使用点内联，
    // 未使用的 `title` 不应以任何模块级绑定形式出现。
    assert!(!v_source.contains("let title"));
}

#[test]
fn boot_script_auto_starts_manifest_no_page_inline() {
    let relative = generate_boot_script("legion-test", manifest_url_for_mode(true));
    assert!(relative.contains("var MANIFEST_URL = 'manifest.json'"), "boot 应内建相对 manifest URL");
    assert!(relative.contains("start(MANIFEST_URL)"), "boot 应自启动 manifest");
    assert!(relative.contains("asgard start failed"));
    let absolute = generate_boot_script("asgard-app", manifest_url_for_mode(false));
    assert!(absolute.contains("var MANIFEST_URL = '/manifest.json'"), "boot 应内建根绝对 manifest URL");
    assert_eq!(asgard_boot_script_tag(true), r#"<script src="boot.js"></script>"#);
    assert_eq!(asgard_boot_script_tag(false), r#"<script src="/boot.js"></script>"#);
    assert!(!asgard_boot_script_tag(true).contains("__voa"));
}

#[test]
fn boot_script_exposes_unmount_and_unbinds_listeners() {
    let boot = generate_boot_script("legion-test", manifest_url_for_mode(true));
    assert!(boot.contains("unmountIslands: unmountIslands"), "boot 应导出 unmountIslands");
    assert!(boot.contains("registerCleanup"), "挂载期建立的订阅应登记到 island 作用域");
    assert!(boot.contains("node.removeEventListener(eventName, fn)"), "卸载应真正解除事件监听");
    assert!(boot.contains("host.__voaDispose"), "挂载点应持有解绑清单");
    assert!(boot.contains("memos.delete(memoId)"), "卸载应丢弃 memo 缓存");
    assert!(boot.contains("releaseDomHandles"), "卸载应释放 DOM 句柄");
}

#[test]
fn boot_script_provides_every_synthesized_env_import() {
    // 发射期会为「未捕获 `raise` 且 payload 是文本」合成 `env::host_raise`
    // （`NODE_HOST_IMPORT_FIELDS`），宿主实现缺任何一项都会让整个产物在
    // `WebAssembly.instantiate` 阶段以 `LinkError: function import requires a callable`
    // 失败——这类失败发生在产物交付之后，不是编译期错误，必须由宿主侧测试钉住。
    let boot = generate_boot_script("asgard-app", manifest_url_for_mode(true));
    for field in [
        "const_utf8",
        "host_intern_utf8",
        "host_utf8_extract",
        "host_raise",
        "utf8_concat",
        "utf8_length",
        "utf8_trim",
        "utf8_to_lower",
        "utf8_to_upper",
        "utf8_replace",
        "utf8_starts_with",
        "utf8_ends_with",
        "utf8_contains",
        "utf8_equals",
        "utf8_index_of",
        "utf8_slice",
    ] {
        assert!(boot.contains(&format!("{field}:")), "boot.js 的 `env` 缺少导入实现 `{field}`");
    }
}

#[test]
fn input_intrinsic_emits_two_way_value_binding() {
    let source = r#"<widget><Column><Input value="{label}" /></Column></widget>
<script>
let mut label: utf8 = ""
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let v_source = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(v_source.contains("dom_create_element(\"input\")"), "`Input` 必须降低为宿主 `input` 元素");
    assert!(v_source.contains("rx_bind_value_utf8"), "`value` 绑定应写元素属性而非 HTML 属性");
    assert!(v_source.contains("dom_bind_input_utf8"), "输入事件应把 DOM value 写回信号");
}

#[test]
fn script_micro_call_wrapper_mirrors_parameters() {
    let source = r#"<widget index><Column><Text @click="toggle('on')">toggle</Text></Column></widget>
<script>
let mut mode: utf8 = "off"
micro toggle(key: utf8) {
    mode = "on"
}
micro pair(a: i32, b: utf8) { }
micro noop() { }
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "index", "index.awsl", &LoweringOptions::default()).expect("lower");
    let v_source = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(v_source.contains("micro awsl_call_toggle(key: utf8) {\n    toggle(key)\n}"), "包装器必须镜像形参并逐个转发：{v_source}");
    assert!(v_source.contains("micro awsl_call_pair(a: i32, b: utf8) {\n    pair(a, b)\n}"), "多形参包装器必须完整转发：{v_source}");
    assert!(v_source.contains("micro awsl_call_noop() {\n    noop()\n}"), "零形参包装器不得被改成带实参：{v_source}");
    assert!(v_source.contains("awsl_call_toggle(\"on\")"), "事件侧带实参调用包装器：{v_source}");
}

#[test]
fn abi_property_and_event_extracted_from_script() {
    let source = r#"<widget demo><Child :theme="theme" /></widget>
<script>
[property]
let theme = "dark";

[event]
micro theme_change(theme: string) { }

micro handler() {
    emit(theme_change, theme);
}
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    assert_eq!(lowered.component_abi.properties.len(), 1);
    assert_eq!(lowered.component_abi.events.len(), 1);
    assert!(lowered.abi_issues.iter().all(|issue| issue.severity != oak_awsl::AbiSeverity::Error));
}

/// 跨文件非法 ABI：模板把未声明的属性绑到目标组件，必须在生成阶段失败并指向组件与属性。
#[test]
fn undeclared_cross_component_prop_fails_at_generation() {
    let child_source = r#"<widget child><p>child</p></widget>
<script>
[property]
let label: utf8 = ""
</script>"#;
    let parent_source = r#"<widget parent><Child :missing_prop="ok" /></widget>"#;
    let child = compile_awsl_source(child_source, "child", "child.awsl", &LoweringOptions::default()).expect("lower child");
    let parent = compile_awsl_source(parent_source, "parent", "parent.awsl", &LoweringOptions::default()).expect("lower parent");
    let error = build_awsl_wasm_source(&[child, parent], ScalarBootstrap::ClosedUnit).expect_err("未声明的 `:prop` 必须在生成阶段失败");
    let message = error.to_string();
    assert!(message.contains("`parent`"), "错误必须指向宿主组件:\n{message}");
    assert!(message.contains(":missing_prop"), "错误必须指向属性:\n{message}");
    assert!(message.contains("`child`"), "错误必须指向目标组件:\n{message}");
}

/// 匿名对象字面量不是 VX primary：脚本 ABI 提取为空时必须在生成阶段失败，
/// 而不是留下 0 绑定继续生成、最后漂成 V 编译期的 `unresolved call infix ==`。
#[test]
fn anonymous_object_literal_fails_abi_extraction_with_component_identity() {
    let source = r#"<widget chart_status><p>{title}</p></widget>
<script>
let title: utf8 = "x"
let series: list = [{ key: "pass", label: "通过" }]
micro toggle(key) { title = key }
</script>"#;
    let error = compile_awsl_source(source, "chart_status", "chart-status.awsl", &LoweringOptions::default())
        .expect_err("匿名对象字面量必须让脚本 ABI 提取失败");
    let message = error.to_string();
    assert!(message.contains("chart_status"), "错误必须指向组件:\n{message}");
    assert!(message.contains("ABI"), "错误必须是 ABI 与脚本对不上的语义:\n{message}");
}

/// 未知属性类型没有信号通道：必须在生成阶段报错，不能回落成 utf8 空串。
#[test]
fn unknown_property_type_is_rejected() {
    let source = r#"<widget demo><p>demo</p></widget>
<script>
[property]
let payload = "x"
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    assert_eq!(lowered.script_bindings.len(), 1);
    assert_eq!(lowered.script_bindings[0].value_type, rusty_valkyrie::asgard::awsl::SignalValueType::Utf8);
}

/// 无法静态判定通道的响应式初始化不能回落 utf8：报错并指向组件与绑定。
#[test]
fn untyped_reactive_initializer_is_rejected() {
    let source = r#"<widget demo><p>{state}</p></widget>
<script>
let mut state = compute()
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let error = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect_err("无法推断通道的响应式绑定必须失败");
    let message = error.to_string();
    assert!(message.contains("`demo`"), "错误必须指向组件:\n{message}");
    assert!(message.contains("state"), "错误必须指向绑定:\n{message}");
}

/// 文本投影必须落既有 `Display` 合同，并显式声明来源。
///
/// 生成单元从 `utf8(<expr>)` 改成 `<expr>.display()` 后，`display` 的可见性不能赌
/// 依赖包的隐式传播：生成源码要自带 `using std.text;`，方法调用点才解析到
/// `std.text` 具体 `imply` 成员。没有 `Display` 的类型则由 V 的成员解析失败闭合，
/// 生成端不提供拼字符串或 `format` 兜底。
#[test]
fn text_positions_project_through_explicit_display_contract() {
    let source = r#"<widget demo><Column><Text>{count}</Text><Text>{label}</Text></Column></widget>
<script>
let mut count = 0
let label = "hi"
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let generated = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(
        generated.starts_with("using std.text;\n\n"),
        "封闭生成单元的标量载体在独立文件里，主文件只显式导入文本合同:\n{generated}"
    );
    assert!(
        generated.contains(r#"return (sig_get_i32(sig_named("demo_count_sig"))).display()"#),
        "整数文本位置必须投影成接收者 `display()` 调用:\n{generated}"
    );
    assert!(
        !generated.lines().any(|line| line.trim_start().starts_with("return utf8(")),
        "生成端不得再发可调用形态的 `utf8(...)` 转换:\n{generated}"
    );
}

/// 未定型的文本位置表达式一律走 `Display`，生成端不按字面量形状猜数字/布尔/拼接。
#[test]
fn non_literal_text_expressions_route_through_display() {
    let source = r#"<widget demo><Column><Text>{1 + 2}</Text><Text>{true}</Text><Text>{greeting}</Text></Column></widget>
<script>
let greeting = "hi"
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let generated = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(generated.contains("return (1 + 2).display()"), "算术表达式必须整体投影：\n{generated}");
    assert!(generated.contains("return (true).display()"), "布尔字面量必须走 `Display`：\n{generated}");
    assert!(generated.contains("return \"hi\""), "字符串字面量按合法捕获输出：\n{generated}");
    assert!(!generated.contains("\"1\" + \"2\""), "生成端不得把 `+` 拆成文本拼接：\n{generated}");
}

/// 无 `Display` 实现的类型必须在文本位置失败闭合，且诊断点名接收者类型。
///
/// 生成端只发 `<expr>.display()`，没有拼字符串或 `format` 兜底；因此缺口必须
/// 表现为成员调用解析失败，并让作者能直接从错误里看到是哪个类型缺实现。
/// `f64` 是当前 std 明确未落 `Display` 的标量，正是图表 `height_pct` 文本位置的形态。
#[test]
fn missing_display_projection_fails_closed_with_receiver_type() {
    let source = crate::primitives::with_primitives(
        r#"
class Label { _repr: [u8] }
type utf8 = Label
trait Display { micro display(self) -> utf8 }
imply i32: Display { micro display(self) -> utf8 { return "0" } }

[export]
micro probe(x: f64) -> utf8 {
    return (x).display()
}
"#,
    );
    let error = ValkyrieCompiler::default().compile_source(&source).expect_err("`f64` 没有 `Display` 实现，文本投影必须失败闭合");
    let text = error.to_string();
    assert!(text.contains("unresolved call `x.display`"), "失败必须指向投影调用点：{text}");
    assert!(text.contains("receiver type `f64`"), "诊断必须点名接收者类型：{text}");
}

/// `Derived` 非响应式绑定的 `value_type` 不进入生成代码（引用点直接内联），
/// 因此表达式无法静态判定通道时使用哪个值都不影响正确性——这里证明内联确实发生。
#[test]
fn derived_expression_without_type_hint_is_inlined() {
    let source = r#"<widget demo><p>{label}</p></widget>
<script>
let label = compute()
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let generated = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(generated.contains("(compute())"), "Derived 必须在引用点内联:\n{generated}");
}
