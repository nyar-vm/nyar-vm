//! 布局/样式合同原型测试：文法 fail-closed、能力矩阵、文本度量自持、字体栈、
//! web_only 移植成本登记与非 dom 后端 fail-closed。

use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, compile_awsl_source},
    layout::{
        Points,
        capability::BackendStage,
        fonts::{Platform, Script, fallback_stack},
        measure::{FontId, StubFontMetrics, TextRun, measure_run},
    },
    style::{Property, Selector, State, StyleContractError, parse_style_contract},
};

fn parse_ok(source: &str) -> rusty_valkyrie::asgard::style::StyleContract {
    parse_style_contract(source).unwrap_or_else(|error| panic!("合同应接受：{error:?}"))
}

fn parse_err(source: &str) -> StyleContractError {
    parse_style_contract(source).expect_err("合同应拒绝")
}

#[test]
fn contract_accepts_three_selector_kinds_and_states() {
    let contract = parse_ok(
        "root { direction: vertical; spacing: 24 }\nslot tabs { direction: horizontal; main-align: space-between }\n#title { text: @hero; color: @ink }\nstate hover on #title { color: @brand }\n",
    );
    assert_eq!(contract.rules.len(), 4);
    assert_eq!(contract.rules[0].selector, Selector::Root);
    assert_eq!(contract.rules[1].selector, Selector::Slot("tabs".into()));
    assert_eq!(contract.rules[2].selector, Selector::Id("title".into()));
    assert_eq!(contract.rules[3].state, Some(State::Hover));
}

#[test]
fn contract_rejects_css_only_syntax() {
    // 标签选择器 / 类选择器 / 通配符 / 后代组合器 / 伪元素一律拒绝。
    assert!(matches!(parse_err("div { spacing: 4 }"), StyleContractError::BadSelector { .. }));
    assert!(matches!(parse_err(".hero { spacing: 4 }"), StyleContractError::BadSelector { .. }));
    assert!(matches!(parse_err("* { spacing: 4 }"), StyleContractError::BadSelector { .. }));
    assert!(matches!(parse_err("root > slot a { spacing: 4 }"), StyleContractError::BadSelector { .. }));
    assert!(matches!(parse_err("root::before { spacing: 4 }"), StyleContractError::BadSelector { .. }));
    assert!(matches!(parse_err("root { spacing: 4 !important }"), StyleContractError::ImportantForbidden));
}

#[test]
fn contract_rejects_unknown_property_and_host_units() {
    // 属性集合封闭：未知属性报错而不是忽略。
    assert!(matches!(parse_err("root { flex-direction: column }"), StyleContractError::UnknownProperty { .. }));
    assert!(matches!(parse_err("root { backdrop-filter: blur(4) }"), StyleContractError::UnknownProperty { .. }));
    // 依赖宿主盒模型的相对单位拒绝；弹性尺寸走合同关键字。
    assert!(matches!(parse_err("root { width: 50% }"), StyleContractError::BadValue { .. }));
    assert!(matches!(parse_err("root { width: 12em }"), StyleContractError::BadValue { .. }));
    assert!(matches!(parse_err("root { spacing: 2vw }"), StyleContractError::BadValue { .. }));
    // 合同关键字与令牌合法。
    assert!(parse_ok("root { width: fill; height: content }").rules[0].declarations.len() == 2);
}

#[test]
fn capability_matrix_marks_stage1_frozen_and_native_approximations() {
    use rusty_valkyrie::asgard::layout::capability::Support;
    use rusty_valkyrie::asgard::style::matrix::{StyleCapability, support};
    // 阶段一定死的方向/间距在四阶段都必须是支持或近似，不得出现不支持。
    for capability in [StyleCapability::Property(Property::Direction), StyleCapability::Property(Property::Spacing)] {
        for stage in [BackendStage::DomHtml, BackendStage::WebView, BackendStage::Native, BackendStage::SelfRendered] {
            assert_ne!(support(capability, stage), Support::Unsupported { reason: "x" }, "方向/间距不得在任何阶段不支持");
        }
    }
    // native 的对齐是近似；文本度量在 native 桩阶段不支持（fail-closed 闸门）。
    assert!(matches!(support(StyleCapability::Property(Property::CrossAlign), BackendStage::Native), Support::Approximate { .. }));
    assert!(matches!(support(StyleCapability::Property(Property::Measure), BackendStage::Native), Support::Unsupported { .. }));
    // web_only 只在 dom/html 支持。
    assert!(matches!(support(StyleCapability::WebOnlyEscape, BackendStage::DomHtml), Support::Supported));
    assert!(matches!(support(StyleCapability::WebOnlyEscape, BackendStage::Native), Support::Unsupported { .. }));
}

#[test]
fn text_measurement_is_owned_by_layout_layer_with_cjk_metrics() {
    // 中文度量单独验证：表意文字全角 1.0em，断行由布局层贪心完成。
    let run = TextRun { text: "十二个节点全部在线平均负载百分之三十四".into(), font: FontId("stage1-system-stack".into()), size: Points(15.0) };
    let metrics = measure_run(&run, Points(90.0), &StubFontMetrics);
    assert!(metrics.lines.len() >= 4, "90pt 宽、15pt 全角字应断出多行：{}", metrics.lines.len());
    assert!(metrics.lines.iter().all(|line| line.width.0 <= 90.0), "行盒宽度不得超过度量约束");
    assert_eq!(metrics.height.0, 15.0 * 1.5 * metrics.lines.len() as f32, "总高 = 行高 × 行数");
    // 拉丁文本按窄表度量，行盒数少于同长中文。
    let latin = TextRun { text: "aaaaaaaaaaaaaaaaaaaaaaaa".into(), font: FontId("stage1-system-stack".into()), size: Points(15.0) };
    let latin_metrics = measure_run(&latin, Points(90.0), &StubFontMetrics);
    assert!(latin_metrics.lines.len() < metrics.lines.len(), "拉丁窄表度量行数应少于中文全角");
}

#[test]
fn font_fallback_stacks_end_with_guaranteed_family() {
    // 每个平台/脚本的栈序末位必须是必有兜底；CJK 栈首选项按平台区分。
    for platform in [Platform::MacOs, Platform::Windows, Platform::Linux, Platform::Android, Platform::Web] {
        for script in [Script::Latin, Script::Cjk] {
            let stack = fallback_stack(script, platform);
            assert_eq!(stack.last(), Some(&"sans-serif"), "栈序末位必须是必有兜底：{stack:?}");
        }
    }
    assert_eq!(fallback_stack(Script::Cjk, Platform::Windows)[0], "Microsoft YaHei");
    assert_eq!(fallback_stack(Script::Cjk, Platform::MacOs)[0], "PingFang SC");
    assert_eq!(fallback_stack(Script::Cjk, Platform::Linux)[0], "Noto Sans CJK SC");
}

#[test]
fn web_only_block_is_registered_and_rejected_off_dom() {
    let contract = parse_ok("root { spacing: 8 }\nweb_only { .legacy { backdrop-filter: blur(4px); } }\n");
    assert_eq!(contract.web_only.len(), 1);
    let entries = rusty_valkyrie::asgard::style::web::porting_cost_entries(&contract);
    assert_eq!(entries.len(), 1, "web_only 必须登记为移植成本条目");
    // dom 后端校验通过；native 降级 fail-closed。
    rusty_valkyrie::asgard::style::web::validate_for_dom(&contract).expect("dom 后端应接受 web_only");
    let error = rusty_valkyrie::asgard::style::native::lower_contract(&contract).expect_err("native 后端必须 fail-closed");
    assert!(error.to_string().contains("web_only"), "诊断必须指明 web_only：{error}");
    // 无 web_only 的合同 native 可降级为样式结构体（不生成 CSS 文本）。
    let plain = parse_ok("root { spacing: 8 }");
    let sheet = rusty_valkyrie::asgard::style::native::lower_contract(&plain).expect("无 web_only 的合同应可降级为结构体");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].handle, "sc-root");
}

#[test]
fn build_rejects_unknown_property_in_component_style() {
    // 组件级 fail-closed：未定义属性在编译入口报错，不进入产物。
    let error = compile_awsl_source(
        "<widget index>\n<Text>x</Text>\n</widget>\n<style>\nroot { flex-wrap: wrap }\n</style>\n",
        "index",
        "index.w",
        &LoweringOptions::default(),
    )
    .expect_err("未定义属性必须编译失败");
    assert!(error.to_string().contains("flex-wrap"), "诊断必须指明被拒属性：{error}");
}

#[test]
fn build_registers_porting_cost_in_manifest() {
    use rusty_valkyrie::asgard::{codegen::{IslandPackageOptions, package_browser_islands}, style::web::porting_cost_entries};
    let component = compile_awsl_source(
        "<widget index>\n<Text>x</Text>\n</widget>\n<style>\nroot { spacing: 8 }\nweb_only { .x { zoom: 1; } }\n</style>\n",
        "index",
        "index.w",
        &LoweringOptions::default(),
    )
    .expect("web_only 在 dom 后端应可编译");
    let temp = tempfile::TempDir::new().expect("临时目录");
    let output = temp.path().join("out");
    package_browser_islands(
        std::slice::from_ref(&component),
        &output,
        &IslandPackageOptions {
            module_stem: "porting".into(),
            wasm_stem: "porting".into(),
            wasm_built: false,
            relative_urls: true,
            css_mode: "per-component".into(),
            mode: "static".into(),
            tailwind_css: None,
            porting_cost: porting_cost_entries(component.style_contract.as_ref().expect("合同存在")),
        },
    )
    .expect("打包应成功");
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(output.join("manifest.json")).expect("读取 manifest")).expect("manifest JSON");
    let cost = manifest.get("porting_cost").and_then(serde_json::Value::as_array).expect("manifest 必须登记移植成本");
    assert_eq!(cost.len(), 1, "web_only 块必须逐条登记");
}
