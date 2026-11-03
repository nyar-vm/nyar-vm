//! AWSL 降级与 JS 胶水测试（自包含样例，不读取外部语言语料）。

use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, lower_component},
    codegen::{asgard_boot_script_tag, build_awsl_wasm_source, generate_boot_script, manifest_url_for_mode},
};

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
