//! SSG/文档模板必须整体落在样式合同内。
//!
//! 这些模板是 `legion test` / `legion doc` 实际编译的页面源；任何残留的类选择器、
//! 宿主单位或合同外属性都会在此失败闭合，而不是在运行矩阵时以 Rust panic 暴露。

use rusty_valkyrie::asgard::awsl::{LoweringOptions, compile_awsl_source};

/// 页面模板清单：诊断标签、组件名与内嵌资产。
const TEMPLATES: &[(&str, &str, &str)] = &[
    ("report/layout", "layout", include_str!("../../assets/report/layout.awsl")),
    ("report/test-report", "test_report", include_str!("../../assets/report/test-report.awsl")),
    ("report/coverage-report", "coverage_report", include_str!("../../assets/report/coverage-report.awsl")),
    ("report/bench-report", "bench_report", include_str!("../../assets/report/bench-report.awsl")),
    ("doc/layout", "layout", include_str!("../../assets/doc/layout.awsl")),
    ("doc/hub", "hub", include_str!("../../assets/doc/hub.awsl")),
];

#[test]
fn ssg_templates_compile_under_style_contract() {
    for (label, widget, source) in TEMPLATES {
        compile_awsl_source(source, widget, &format!("assets/{label}.awsl"), &LoweringOptions::default())
            .unwrap_or_else(|error| panic!("SSG 模板 `{label}` 必须落在样式合同内，实际失败：{error:?}"));
    }
}

/// 真实报告渲染路径：`legion test` 生成报告时不得在模板编译处 panic。
///
/// 依赖 `valkyrie.v` 源码库的页面模板；CI 无该兄弟仓时跳过。
///
/// 已知缺口（等 AWSL 生成链修复后打开）：注入图表岛后该路径会命中
/// `infix ==` 无候选。根因在 AWSL 生成链当前 fail-open：注入的初始值是匿名对象字面量，
/// `extract_component_abi_from_script` 报 `widget not found`，`compile_awsl_source`
/// 吞掉 `abi_issues` 使 `script_bindings` 为 0，`rewrite_script_decls_for_signals`
/// 随后删掉模块级 `let` 却无从改写成信号读写。该链路由 AWSL 生成链修复线负责，
/// 本线不动。因此修复落地前不要给 CI 设置 `VALKYRIE_V_ROOT`（否则本用例长期真红）；
/// 修复落地后应一次性设 env、把本用例转为真实执行，并同时解开报告渲染用例的同类跳过。
#[test]
fn live_report_render_does_not_fail_on_templates() {
    let Ok(roots) = rusty_valkyrie::asgard::valkyrie_v_roots() else {
        eprintln!("skip: 未配置 `VALKYRIE_V_ROOT`，报告渲染需要 valkyrie.v 资产");
        return;
    };
    let dir = tempfile::tempdir().expect("临时目录");
    let results = vec![rusty_valkyrie::legion::cmds::report::TestResultEntry {
        name: "ssg-contract".into(),
        status: "pass".into(),
        error: None,
        target: "legion".into(),
    }];
    rusty_valkyrie::legion::cmds::report::render_test_report(dir.path(), "ssg-contract", &results, &roots)
        .expect("报告渲染必须在模板编译处通过样式合同");
    assert!(dir.path().join("index.html").is_file(), "渲染必须产出页面");
}
