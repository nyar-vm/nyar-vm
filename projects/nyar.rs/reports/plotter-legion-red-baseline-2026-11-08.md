# 图表岛 legion 红点验收基线（2026-11-08）

**用途**：文本身份（内建 `Utf8` 与名义 `Utf8Text` 对称等价）落地后，按本表复跑对照，
确认哪些红点自动转绿、哪些仍需各自归属线处理。

## 复跑坐标

| 项 | 值 |
| --- | --- |
| 编译器仓 | `nyar-vm.rs`，`dev` `6ab54e39`（本次工作树在其上加 `e0e9e283`） |
| 消费方仓 | `valkyrie.v`，`dev` `3f65d7c` |
| 环境变量 | `VALKYRIE_V_ROOT=E:\victory 胜利女神\.wt-plotter-browser` |
| 命令 | `cargo test -q -p rusty-valkyrie --test legion`（cwd=`projects/nyar.rs`） |
| 结果 | **187 passed / 12 failed / 1 ignored** |

## A. 文本身份同源红（9 条）

共同首个失败点：`报告图表岛 WASM 编译失败: unresolved call \`sig_set_utf8\` in pin at
asgard/generated.v:<offset>: 1 candidate(s) share the name, but none matches the argument types`
——生成单元的 ABI 位置按内建 `Utf8` 解析，字段值却是名义 `Utf8Text`。

| 测试 | 首个失败点 | 归属 | 预期 |
| --- | --- | --- | --- |
| `legion_cmds_report::test_report_html_structure` | `legion_cmds_report.rs:45` 的 `render_test_report` unwrap，island 编译报 `sig_set_utf8` | 文本身份线 | 自动转绿 |
| `legion_cmds_report::test_report_standalone_html_is_self_contained` | `legion_cmds_report.rs:151`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_report::coverage_report_html_structure` | `legion_cmds_report.rs:96`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_report::coverage_report_standalone_html_is_self_contained` | `legion_cmds_report.rs:182`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_report::bench_report_html_structure` | `legion_cmds_report.rs:125`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_report::bench_report_standalone_html_is_self_contained` | `legion_cmds_report.rs:200`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_cov::coverage_generates_json_and_html` | `legion_cmds_cov.rs:55`，同上 | 文本身份线 | 自动转绿 |
| `legion_cmds_test::test_filter_skips_non_matching` | `legion_cmds_test.rs:238` unwrap，island 编译报 `sig_set_utf8` | 文本身份线 | 自动转绿 |
| `legion_cmds_test::test_command_writes_html_report` | `legion_cmds_test.rs:111`「missing HTML report」（报告构建先 abort，未写 HTML），根因同上 | 文本身份线 | 自动转绿（HTML 落盘后断言才可执行） |

## B. dev 上 OVM ABI 阶段三红（3 条）

共同首个失败点：`多个分区同时产出 OVM ABI sidecar：\`<app>::export__default\` 与
\`<app>::functions\`；拒绝合并歧义 ABI 合同`（sidecar 合并歧义）。

| 测试 | 首个失败点 | 归属 | 预期 |
| --- | --- | --- | --- |
| `generic_struct_construct_context_e2e::generic_struct_construct_uses_expected_type_in_builtin_ovm` | `generic_struct_construct_context_e2e.rs:87`，sidecar 歧义（`generic_construct_context_app`） | OVM ABI 阶段三 | 不转绿 |
| `legion_test_ovm_builtin_e2e::every_test_annotation_yields_one_run_contract_with_annotated_main` | `legion_test_ovm_builtin_e2e.rs:111`，sidecar 歧义（`contract_count_app`） | OVM ABI 阶段三 | 不转绿 |
| `variant_auto_constructor_option_e2e::variant_auto_constructor_entries_execute_in_builtin_ovm` | `variant_auto_constructor_option_e2e.rs:128`，sidecar 歧义（`variant_auto_constructor_app`） | OVM ABI 阶段三 | 不转绿 |

## C. 相对「14 条」口径的已转绿 2 条（保留对照）

上一口径 5 条 ABI 红中的「函数类型导出」两条，已被 `6ab54e39`（Keep reachable callees out of
the declared `ovm` export surface）关掉，复跑已通过：

- `legion_ovm_closure_p2_e2e::ovm_named_lambda_without_capture_executes_unchanged`
  （原失败：`库导出 \`app::apply\` 参数 0 类型 \`fn(i64) -> i64\` 没有库导出 ABI 载体`）
- `legion_ovm_closure_p2_e2e::ovm_hashset_from_list_callback_with_capture_executes`
  （原失败：`库导出 \`std::iter#189\` 参数 1 类型 \`fn(i32) -> unit\` 没有库导出 ABI 载体`）

## 验收判据

1. 文本身份线合入后复跑：A 组 9 条应全部转绿；B 组 3 条保持红且归 OVM ABI 阶段三。
2. 若 A 组仍有红，记录新的首个失败点并新开归属，不回退本表。
3. 本表只登记失败坐标，不放宽断言、不加 `#[ignore]`、不引入兜底。
