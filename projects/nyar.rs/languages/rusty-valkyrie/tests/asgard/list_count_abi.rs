use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, lower_component},
    codegen::{ScalarBootstrap, build_awsl_wasm_source, generate_boot_script, manifest_url_for_mode},
};

const FIXTURE: &str = r#"<widget index>
<Column>
    <loop item in series>
        <Text class="row">{item}</Text>
    </loop>
</Column>
</widget>
<script>
let series = ["第一条", "第二条", "第三条"]
</script>"#;

fn generated_source() -> String {
    let root = AwslParser::parse_root(FIXTURE).expect("parse loop fixture");
    let lowered = lower_component(&root, "index", "index.awsl", &LoweringOptions::default()).expect("lower loop fixture");
    build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate loop fixture").source
}

fn items_export_block(source: &str) -> &str {
    let start = source.find("micro awsl_items_").expect("`<loop>` 必须生成 items 导出");
    let end = start + source[start..].find("\n}\n").expect("items 导出必须闭合");
    &source[start..end]
}

/// 计数 ABI：`items` 导出只返回条目数量（`usize`），生成源码里不得再出现伪造的 `list` 类型。
#[test]
fn loop_items_export_returns_count_without_list_type() {
    let source = generated_source();
    let block = items_export_block(&source);
    assert!(block.contains("(): usize {"), "items 导出必须以 `usize` 返回计数:\n{block}");
    assert!(block.contains(").length()"), "items 导出必须只取条目数量:\n{block}");
    assert!(!block.contains(": list"), "items 导出不得再声明伪造的 `list` 类型:\n{block}");
}

/// 宿主侧按计数消费：`rxBindLoop` 只读计数并按下标重取元素，`Array.isArray` 死检查已移除。
#[test]
fn boot_rx_bind_loop_consumes_count_only() {
    let boot = generate_boot_script("loop-count-abi", manifest_url_for_mode(true));
    assert!(boot.contains("var count = Number(callWasm(hostText(itemsExport))) || 0"), "boot 必须把 items 导出按计数读取");
    assert!(boot.contains("for (var i = 0; i < count; i++)"), "boot 必须按计数循环挂载");
    assert!(boot.contains("callWasm(hostText(mountExport), container, i + 1)"), "boot 必须传 1 基序号由 wasm 重取元素");
    assert!(!boot.contains("Array.isArray"), "`Array.isArray(items)` 死检查必须移除");
}
