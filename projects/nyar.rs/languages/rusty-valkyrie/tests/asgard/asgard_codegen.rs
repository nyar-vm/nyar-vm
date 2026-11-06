//! AWSL 降级与 JS 胶水测试（自包含样例，不读取外部语言语料）。

use oak_awsl::AwslParser;
use rusty_valkyrie::ValkyrieCompiler;
use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, lower_component},
    codegen::{asgard_boot_script_tag, build_awsl_wasm_source, generate_boot_script, manifest_url_for_mode},
};

/// 生成单元（`awsl_hydrate_*` 一类宿主调用）的最小形态：签名用裸 `i32`/`utf8` 拼写，
/// 调用点直接传整数字面量。生成单元不继承依赖包的语义导出，因此必须自带数值标量声明；
/// `bool` 是编码型内建，不需要也不应重声明。
const GENERATED_UNIT_SCALAR_SHAPE: &str = r#"[primitive("std::primitive::i32")]
structure i32 { }

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
    let v_source = build_awsl_wasm_source(&[lowered]);
    assert!(
        v_source.contains("[primitive(\"std::primitive::i32\")]"),
        "生成单元必须自带 `i32` 标量声明"
    );
    assert!(
        !v_source.contains("structure bool"),
        "`bool` 是编码型内建，生成单元不得重声明"
    );
    ValkyrieCompiler::default()
        .compile_source(GENERATED_UNIT_SCALAR_SHAPE)
        .expect("标量声明在场时生成单元必须编译通过");
}

/// 反例：剥掉标量声明后，同一调用点的字面量实参必须结构化失败，不得静默选型。
#[test]
fn generated_unit_without_scalar_declarations_rejects_literal_arguments() {
    let without = GENERATED_UNIT_SCALAR_SHAPE.replacen(
        "[primitive(\"std::primitive::i32\")]\nstructure i32 { }\n\n",
        "",
        1,
    );
    assert!(!without.contains("structure i32 { }"), "反例必须真正移除标量声明");
    let error = ValkyrieCompiler::default()
        .compile_source(&without)
        .expect_err("缺少标量声明时字面量实参必须结构化失败");
    let text = format!("{error:?}");
    assert!(text.contains("UnresolvedCall"), "失败必须是调用解析诊断：{text}");
    assert!(text.contains("rx_bind_text_utf8"), "诊断必须指向宿主调用：{text}");
    assert!(text.contains("NoViableOverload"), "失败原因必须是无可行重载：{text}");
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
    let v_source = build_awsl_wasm_source(&[lowered]);
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
fn input_intrinsic_emits_two_way_value_binding() {
    let source = r#"<widget><Column><Input value="{label}" /></Column></widget>
<script>
let mut label: utf8 = ""
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo");
    let v_source = build_awsl_wasm_source(&[lowered]);
    assert!(v_source.contains("dom_create_element(\"input\")"), "`Input` 必须降低为宿主 `input` 元素");
    assert!(v_source.contains("rx_bind_value_utf8"), "`value` 绑定应写元素属性而非 HTML 属性");
    assert!(v_source.contains("dom_bind_input_utf8"), "输入事件应把 DOM value 写回信号");
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
