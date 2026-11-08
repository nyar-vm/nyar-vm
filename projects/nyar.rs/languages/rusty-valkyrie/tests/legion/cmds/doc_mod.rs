//! Migrated from `src/cmds/doc/mod.rs` (`tests`).

use rusty_valkyrie::legion::cmds::doc::*;
use rusty_valkyrie::legion::cmds::doc::assets;

use serde_json::json;

#[test]
fn hub_template_renders() {
    let data = json!({
        "title": "文档",
        "doc_href": "doc/index.html",
        "doc_desc": "测试",
    });
    let result = rusty_valkyrie::asgard::ssg::render_awsl_static(assets::embedded_hub_awsl(), "hub", "hub.awsl", &data).expect("render awsl static");
    assert!(result.html.contains("doc/index.html"));
}

