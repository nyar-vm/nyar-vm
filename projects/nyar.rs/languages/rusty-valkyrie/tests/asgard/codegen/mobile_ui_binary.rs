//! Migrated from `src/codegen/mobile_ui_binary.rs` (`tests`).

use rusty_valkyrie::asgard::codegen::mobile_ui_binary::*;

use rusty_valkyrie::asgard::awsl::{LoweringOptions, RenderModule, lower_component};
use oak_awsl::AwslParser;

#[test]
fn counter_ui_bin_snapshot() {
    let source = r#"<widget counter>
<Column>
    <Text>{count}</Text>
    <Button @click="on_tap">+1</Button>
</Column>
</widget>
<script>
let mut count: i32 = 0
micro on_tap() {
    count = count + 1
}
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse awsl");
    let component = lower_component(&root, "counter", "counter.awsl", &LoweringOptions::default()).expect("lower");
    let blob = encode_mobile_ui_package(&[component]);
    assert!(blob.starts_with(UI_BIN_MAGIC));
    assert_eq!(blob[UI_BIN_MAGIC.len()], UI_BIN_VERSION_V3);
    let text = String::from_utf8_lossy(&blob);
    assert!(text.contains("counter"));
    assert!(text.contains("Column"));
    assert!(text.contains("count"));
    assert!(text.contains("on_tap"));

    let decoded = decode_mobile_ui_package(&blob).expect("decode v3 baseline");
    assert_eq!(decoded.version, UI_BIN_VERSION_V3);
    assert_eq!(decoded.components.len(), 1);
    assert_eq!(decoded.components[0].name, "counter");

    // 宿主分派依赖 payload 里的 widget 身份码，而不是重新匹配标签字符串。
    use rusty_valkyrie::asgard::awsl::render_ir::{IntrinsicWidget, RenderNode};
    let module = &decoded.components[0].render_ir;
    let roots = module.roots.clone();
    let widget_of = |id| match module.node(id) {
        RenderNode::Element(element) => element.widget,
        _ => None,
    };
    assert_eq!(roots.iter().find_map(|id| widget_of(*id)), Some(IntrinsicWidget::Column), "root must carry the Column identity");
    let column = match module.node(roots[0]) {
        RenderNode::Element(element) => module.region(element.children).nodes.clone(),
        other => panic!("expected the Column element, got {other:?}"),
    };
    assert!(
        column.iter().any(|id| widget_of(*id) == Some(IntrinsicWidget::Text)),
        "Text identity must be serialized instead of matched from the tag"
    );
    assert!(
        column.iter().any(|id| widget_of(*id) == Some(IntrinsicWidget::Button)),
        "Button identity must be serialized instead of matched from the tag"
    );
}

#[test]
fn rejects_package_without_container_tag() {
    let mut blob = Vec::new();
    blob.extend_from_slice(UI_BIN_MAGIC);
    blob.extend_from_slice(&1u32.to_le_bytes());
    write_string(&mut blob, "route");
    write_string(&mut blob, "widget");
    write_bindings(&mut blob, &[]);
    write_ir(&mut blob, &RenderModule::empty());
    assert!(decode_mobile_ui_package(&blob).is_err(), "缺失容器标签的负载必须失败");
}

