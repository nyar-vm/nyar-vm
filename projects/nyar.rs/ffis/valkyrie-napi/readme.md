# valkyrie-napi

Valkyrie 产品面的 Node-API（**N-API**）原生绑定。构造：`cargo build -p valkyrie-napi --release`。

## 能力边界

本 crate 是**结构化能力适配层**，不是 Rust CLI 的动态入口：

- 导出的函数只接收结构化请求与选项，返回结构化结果或 N-API 异常。
- 不导出 `argv`、`product + argv`、命令字符串或“把所有命令转发进去”的超级参数。
- 不保留只返回“已移除”错误的存根。`legionRun` / `legionRunCaptured` / `legionParse` /
  `runValkyrieCli` 已从导出表移除。
- 产品选择、命令解析、帮助/版本文案与退出码属于 TypeScript CLI 层，native 层不打印 CLI 文案。

当前导出面：Asgard 静态构建（`asgardBuildStatic`）、Legion `legionPlan` / `legionBuild` /
`legionTest`、artifact 与 OVM 查询，以及 `legionSpyRun`。

编译语义仍在 `rusty-valkyrie`。Asgard 的构建内容由项目根 `asgard.config.v` 的有类型求值决定，
TypeScript CLI 只负责定位项目并调用单一结构化构建能力。
