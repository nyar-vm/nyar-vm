# `rusty-valkyrie/src`

这里是 `rusty-valkyrie` 单一 Cargo 包的实现根。模块按职责组织：`compiler` 处理语言语义，`legion` 处理工程，`asgard` 处理多端交付，`utilities` 提供跨模块共享的基础服务。

```mermaid
flowchart TB
  source[Source project]
  legion[legion\nworkspace / build / test]
  asgard[asgard\nAWSL / render / delivery]
  compiler[compiler\nsyntax / semantic / HIR / MIR]
  utilities[utilities\ndiagnostics / text]
  emitter[nyar-emitter]

  source --> legion
  source --> asgard
  legion --> compiler
  asgard --> compiler
  compiler -->|Valkyrie × 目标适配| emitter
  legion --> utilities
  asgard --> utilities
```

## 模块

| 目录 | 回答的问题 |
|------|------------|
| [`compiler/`](compiler/readme.md) | Oak 语法接入、模板展开、语义分析、HIR、Semantic MIR 和 Nyar 编译适配 |
| [`legion/`](legion/readme.md) | 工程清单、依赖解析、源码快照、构建计划、缓存、测试和产物管理 |
| [`asgard/`](asgard/readme.md) | AWSL/Voa 解析、RenderIR、平台代码生成、打包和部署 |
| [`utilities/`](utilities/readme.md) | 诊断投影、报告格式化和无领域语义的文本服务 |

语言编译器拥有 Valkyrie AST 接入、HIR/MIR 与语义。Nyar 提供可复用工具与中性编排，各平台拥有计划、LIR 和运行时包装。适配器可以使用平台公开的受验证 builder/service，不自行复制指令与容器模型。当前 canonical 专用载荷仍在迁移，不能因位于 Nyar crate 就被视为跨语言合同。

## 对外入口

- `rusty_valkyrie::api`：编译器和语言服务的库入口。
- `rusty_valkyrie::legion`：工程规划、构建和测试的库入口。
- `rusty_valkyrie::asgard`：Voa/AWSL 交付能力的库入口。
- `src/bin/legion.rs`、`src/bin/asgard.rs`：命令行入口。
