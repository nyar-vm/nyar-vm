# compiler / services

**语言服务**：补全、跳转、引用、重命名、语义高亮、格式化注册等。与正式构建共享 `compiler` 的输入快照、作用域与类型事实。

## 子模块

| 模块 | 作用 |
|------|------|
| `highlight` | 语义高亮 |
| `lint` | 源码 lint 入口 |
| `format_provider` | 向 `nyar-format` 注册 Valkyrie 打印/格式化 provider |

## 构建 vs 编辑器

| 请求 | 准入 |
|------|------|
| `compile` job | 入口语义闭包完成 + 严格阶段合同 |
| 编辑器 query | 允许不完整源码；结果须标明缺失/错误，不能冒充构建成功载荷 |

服务通过 `driver` 的查询接口拉取事实，**不** 维护第二套 analyzer 或并行 AST 索引。

```mermaid
flowchart LR
  IDE[编辑器 / LSP]
  svc[services]
  drv[driver]
  sem[semantic 事实]

  IDE --> svc --> drv --> sem
```

Oak 仍生产 formatter 所需的语法事实与 edits；不在 `utilities` 再实现一套解析器。
