//! 终端 V 生成器合同测试：生成物必须匹配冻结的 `std.terminal` 会话外形与
//! `asgard.terminal` 产品入口（session 参数、类型化状态句柄、函数值回调）。
//!
//! 真实语义合同：处理器体从脚本事实生成类型化槽补丁与重挂载；循环生成遍历
//! 全部元素的真实迭代；条件分支保留全部节点。无法生成真实语义的构造
//! （空处理器、无法接线的循环）显式报错拒绝，禁止伪成功输出。

use rusty_valkyrie::asgard::codegen::terminal_v_render::*;

use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::awsl::{LoweredComponent, LoweringOptions, lower_component};

fn lower(source: &str) -> LoweredComponent {
    let root = AwslParser::parse_root(source).expect("parse");
    lower_component(&root, "Test", "test.awsl", &LoweringOptions::default()).expect("lower")
}

fn render(component: &LoweredComponent) -> String {
    render_terminal_component_v(component).expect("generate terminal v")
}

#[test]
fn generates_session_builder_with_awsl_column() {
    let component = lower("<widget><Column><Text>Hi</Text></Column></widget>");
    let v = render(&component);
    assert!(v.contains("micro awsl_build_test(session: mut TuiSession) -> ArrayList<Widget>"), "缺少带 session 的控件树构造函数: {v}");
    assert!(v.contains("awsl_column"), "缺少 awsl_column: {v}");
    assert!(v.contains("awsl_text"), "缺少 awsl_text: {v}");
}

#[test]
fn no_binding_registration_channel() {
    let component = lower(
        r#"<widget><Text>{count}</Text></widget>
<script>
let mut count: i32 = 0
</script>"#,
    );
    let v = render(&component);
    assert!(!v.contains("tui_register_binding"), "旧绑定登记通道未清退: {v}");
    assert!(!v.contains("awsl_init_"), "旧 init 入口未清退: {v}");
}

#[test]
fn main_entry_passes_builder_function_value() {
    let component = lower("<widget><Text>Hi</Text></widget>");
    let v = render(&component);
    assert!(v.contains("micro awsl_main()"), "缺少 main 函数: {v}");
    assert!(v.contains("asgard_terminal_run(awsl_build_test)"), "main 必须把控件树构造函数以函数值交给产品入口: {v}");
    assert!(!v.contains("asgard_terminal_run(roots)"), "旧 roots 入口未清退: {v}");
}

#[test]
fn dynamic_text_evaluates_directly_without_string_channel() {
    let component = lower(
        r#"<widget><Text>Count: {count}</Text></widget>
<script>
let mut count: i32 = 0
</script>"#,
    );
    let v = render(&component);
    assert!(!v.contains("tui_binding_get"), "旧绑定字符串读取通道未清退: {v}");
    // 整数响应式绑定读 `IntBinding` 类型化槽（经运行帧按显示名换取），再走 Display 投影。
    assert!(
        v.contains("awsl_text(\"Count: \" ++ (session.int(AsgardTuiBindings.int(session, \"count\", 0))).display())"),
        "整数绑定文本应经类型化整数槽读取: {v}"
    );
}

#[test]
fn int_binding_text_reads_typed_session_slot_with_negative_initial() {
    let component = lower(
        r#"<widget><Text>{offset}</Text></widget>
<script>
let mut offset: i32 = -7
</script>"#,
    );
    let v = render(&component);
    assert!(
        v.contains("(session.int(AsgardTuiBindings.int(session, \"offset\", -7))).display()"),
        "整数槽初值须承载负字面量（带符号 i32 合同）: {v}"
    );
}

#[test]
fn bool_binding_text_reads_typed_session_slot() {
    let component = lower(
        r#"<widget><Text>{agree}</Text></widget>
<script>
let mut agree: bool = false
</script>"#,
    );
    let v = render(&component);
    assert!(
        v.contains("(session.flag(AsgardTuiBindings.flag(session, \"agree\", false))).display()"),
        "布尔绑定文本应经运行帧换取类型化槽句柄读取: {v}"
    );
}

#[test]
fn button_emits_function_value_callback() {
    let component = lower(
        r#"<widget><Button @click="on_tap">Tap</Button></widget>
<script>
let mut tapped: bool = false
micro on_tap() {
    tapped = !tapped
}
</script>"#,
    );
    let v = render(&component);
    assert!(v.contains("awsl_button(\"Tap\", awsl_call_on_tap)"), "按钮必须以函数值传入回调: {v}");
    assert!(v.contains("micro awsl_call_on_tap(session: mut TuiSession) -> unit"), "回调签名必须带 session: {v}");
    assert!(!v.contains("tui_remount"), "旧重挂载通道未清退: {v}");
    assert!(!v.contains("\"on_tap\""), "回调不得是字符串名: {v}");
}

#[test]
fn handler_body_generates_real_slot_patch_and_remount() {
    let component = lower(
        r#"<widget><Button @click="on_add">Add</Button><Text>{count}</Text></widget>
<script>
let mut count: i32 = 0
micro on_add() {
    count = count + 1
}
</script>"#,
    );
    let v = render(&component);
    // 处理器体必须是真实状态变更：句柄登记 + 类型化槽补丁 + 重挂载刷新展示文本。
    assert!(v.contains("let slot_count: IntBinding = AsgardTuiBindings.int(session, \"count\", 0)"), "处理器必须经运行帧换取类型化槽句柄: {v}");
    assert!(v.contains("session.patch_int(slot_count, session.int(slot_count) + 1)"), "处理器必须生成真实整数槽补丁: {v}");
    assert!(v.contains("asgard_terminal_remount(session, awsl_build_test)"), "处理器必须重挂载刷新展示文本: {v}");
    assert!(!v.contains("# 用户逻辑"), "注释桩假实现未清退: {v}");
}

#[test]
fn empty_handler_is_rejected() {
    let component = lower(
        r#"<widget><Button @click="on_tap">Tap</Button></widget>
<script>
let mut count: i32 = 0
micro on_tap() {
}
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("空处理器必须显式拒绝，禁止伪成功输出");
    assert!(error.to_string().contains("on_tap"), "拒绝诊断必须点名处理器: {error}");
    assert!(error.to_string().contains("空"), "拒绝诊断必须说明空处理器: {error}");
}

#[test]
fn missing_handler_micro_is_rejected() {
    let component = lower(
        r#"<widget><Button @click="on_tap">Tap</Button></widget>
<script>
let mut count: i32 = 0
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("脚本中不存在的处理器必须显式拒绝");
    assert!(error.to_string().contains("on_tap"), "拒绝诊断必须点名处理器: {error}");
}

#[test]
fn untranslatable_handler_statement_is_rejected() {
    let component = lower(
        r#"<widget><Button @click="on_tap">Tap</Button></widget>
<script>
let mut count: i32 = 0
micro on_tap() {
    log_tap()
}
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("无法翻译的处理器语句必须显式拒绝");
    assert!(error.to_string().contains("log_tap"), "拒绝诊断必须带上无法翻译的语句: {error}");
}

#[test]
fn handler_with_parameters_is_rejected() {
    let component = lower(
        r#"<widget><Button @click="on_tap">Tap</Button></widget>
<script>
let mut count: i32 = 0
micro on_tap(step: i32) {
    count = count + step
}
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("带形参的处理器无法接线到固定回调签名，必须显式拒绝");
    assert!(error.to_string().contains("on_tap"), "拒绝诊断必须点名处理器: {error}");
}

#[test]
fn button_without_event_uses_noop_callback() {
    let component = lower(r#"<widget><Button>Tap</Button></widget>"#);
    let v = render(&component);
    assert!(v.contains("awsl_button(\"Tap\", awsl_noop_test)"), "缺事件按钮应落到空操作函数值: {v}");
    assert!(v.contains("micro awsl_noop_test(session: mut TuiSession) -> unit"), "缺少空操作回调定义: {v}");
    assert!(!v.contains("awsl_button(\"Tap\", \"\")"), "不得用空字符串占位回调: {v}");
}

#[test]
fn list_and_item_generate_widget_calls() {
    let component = lower(
        r#"<widget><List><Item @click="on_a">A</Item><Item @click="on_b">B</Item></List></widget>
<script>
let mut chosen: utf8 = ""
micro on_a() {
    chosen = "a"
}
micro on_b() {
    chosen = "b"
}
</script>"#,
    );
    let v = render(&component);
    assert!(v.contains("awsl_list"), "缺少 awsl_list: {v}");
    assert!(v.contains("awsl_item(\"A\", awsl_call_on_a)"), "缺少 awsl_item 函数值回调: {v}");
    assert!(v.contains("awsl_item(\"B\", awsl_call_on_b)"), "缺少 awsl_item 函数值回调: {v}");
    assert!(v.contains("session.patch_choice(slot_chosen, \"a\")"), "选项槽处理器必须生成真实补丁: {v}");
}

#[test]
fn loop_generates_real_iteration_over_all_elements() {
    let component = lower(
        r#"<widget><List><loop item="entry" :each="{rows}"><Text>{entry}</Text></loop></List></widget>
<script>
let rows = ["a", "b", "c"]
</script>"#,
    );
    let v = render(&component);
    // 循环必须是真实迭代：遍历全部元素，循环体控件逐项构建并收集。
    assert!(v.contains("loop entry in ([\"a\", \"b\", \"c\"]) {"), "循环必须生成遍历全部元素的真实迭代: {v}");
    assert!(v.contains("(entry).display()"), "循环体文本必须读取迭代变量: {v}");
    assert!(v.contains(".push("), "循环体控件必须逐项收集进列表: {v}");
    assert!(!v.contains("# loop:"), "空列表 + 循环说明的假实现未清退: {v}");
}

#[test]
fn loop_with_empty_body_is_rejected() {
    let component = lower(
        r#"<widget><List><loop item="entry" :each="{rows}"></loop></List></widget>
<script>
let rows = ["a"]
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("空循环体无法生成真实迭代，必须显式拒绝");
    assert!(error.to_string().contains("循环体为空"), "拒绝诊断必须说明空循环体: {error}");
}

#[test]
fn loop_over_scalar_slot_is_rejected() {
    let component = lower(
        r#"<widget><List><loop item="entry" :each="{count}"><Text>{entry}</Text></loop></List></widget>
<script>
let mut count: i32 = 0
</script>"#,
    );
    let error = render_terminal_component_v(&component).expect_err("标量类型化槽没有可迭代事实，必须显式拒绝");
    assert!(error.to_string().contains("count"), "拒绝诊断必须点名循环源绑定: {error}");
}

#[test]
fn if_branch_keeps_all_nodes() {
    let component = lower(
        r#"<widget><if :condition="shown"><Text>One</Text><Text>Two</Text><Text>Three</Text></if></widget>
<script>
let mut shown: bool = false
</script>"#,
    );
    let v = render(&component);
    // 条件分支必须保留并生成全部节点：三个文本控件都出现在 then 分支。
    assert!(v.contains("awsl_text(\"One\")"), "then 分支首节点丢失: {v}");
    assert!(v.contains("awsl_text(\"Two\")"), "then 分支后续节点丢失: {v}");
    assert!(v.contains("awsl_text(\"Three\")"), "then 分支后续节点丢失: {v}");
    let column_count = v.matches("awsl_column(").count();
    assert!(column_count >= 1, "多节点分支必须包进容器: {v}");
}

#[test]
fn checkbox_registers_typed_handle_via_session() {
    let component = lower(
        r#"<widget><Checkbox :binding="agree">同意</Checkbox></widget>
<script>
let mut agree: bool = false
</script>"#,
    );
    let v = render(&component);
    assert!(v.contains("awsl_checkbox(\"同意\", \"agree\", false, session)"), "复选框必须带显示名初值与 session 换取类型化句柄: {v}");
    assert!(!v.contains("widget_checkbox"), "旧 widget_checkbox 调用未清退: {v}");
}

#[test]
fn radio_registers_choice_handle_via_session() {
    let component = lower(
        r#"<widget><Radio :binding="color" :option="red">Red</Radio></widget>
<script>
let mut color: utf8 = "red"
</script>"#,
    );
    let v = render(&component);
    assert!(v.contains("awsl_radio(\"Red\", \"color\", \"red\", session)"), "单选钮必须带显示名选项值与 session: {v}");
    assert!(!v.contains("widget_radio"), "旧 widget_radio 调用未清退: {v}");
}

#[test]
fn box_generates_awsl_box() {
    let component = lower("<widget><Box><Text>Content</Text></Box></widget>");
    let v = render(&component);
    assert!(v.contains("awsl_box"), "缺少 awsl_box: {v}");
}

#[test]
fn generated_source_has_no_legacy_protocol_symbols() {
    let component = lower(
        r#"<widget><Column>
<Text>{count}</Text>
<Button @click="on_add">Add</Button>
<Checkbox :binding="agree">Agree</Checkbox>
<Radio :binding="color" :option="red">Red</Radio>
</Column></widget>
<script>
let mut count: i32 = 0
let mut agree: bool = false
let mut color: utf8 = "red"
micro on_add() {
    count = count + 1
}
</script>"#,
    );
    let v = render(&component);
    for legacy in [
        "tui_register_binding",
        "tui_binding_get",
        "tui_binding_set",
        "tui_binding_truthy",
        "tui_remount",
        "widget_column",
        "widget_row",
        "widget_box",
        "widget_list",
        "widget_text",
        "widget_button",
        "widget_item",
        "widget_checkbox",
        "widget_radio",
        "asgard_terminal_run(roots)",
    ] {
        assert!(!v.contains(legacy), "旧协议符号 `{legacy}` 残留在生成路径: {v}");
    }
}

#[test]
fn golden_session_shaped_component() {
    let component = lower(
        r#"<widget><Column><Text>Count: {count}</Text><Button @click="on_add">Add</Button></Column></widget>
<script>
let mut count: i32 = 0
micro on_add() {
    count = count + 1
}
</script>"#,
    );
    let v = render(&component);
    let expected = r#"# 控件树构造：接收 session；交互状态经当前运行帧换取类型化槽句柄，展示文本直接求值。
micro awsl_build_test(session: mut TuiSession) -> ArrayList<Widget> {
    let roots: ArrayList<Widget> = ArrayList::new(16)
    let children_0: ArrayList<Widget> = ArrayList::new(8)
    let w1: Widget = awsl_text("Count: " ++ (session.int(AsgardTuiBindings.int(session, "count", 0))).display())
    children_0.push(w1)
    let w2: Widget = awsl_button("Add", awsl_call_on_add)
    children_0.push(w2)
    let w3: Widget = awsl_column(children_0)
    roots.push(w3)
    return roots
}

# 事件处理函数 `on_add`：真实状态变更——经运行帧句柄写类型化槽，随后重挂载刷新展示文本。
micro awsl_call_on_add(session: mut TuiSession) -> unit {
    let slot_count: IntBinding = AsgardTuiBindings.int(session, "count", 0)
    session.patch_int(slot_count, session.int(slot_count) + 1)
    asgard_terminal_remount(session, awsl_build_test)
}

# 主入口：把带 session 的控件树构造函数交给产品入口，会话由产品入口创建。
micro awsl_main() -> i32 {
    return asgard_terminal_run(awsl_build_test)
}
"#;
    assert_eq!(v, expected);
}
