# compiler / semantic

**同一套语义事实系统**的协作模块：声明、可见性、类型、绑定、evidence 与入口需求在固定点中互相推进，而不是按目录顺序线性「先全绑定再全推断」。

## 固定点（概念）

```mermaid
flowchart TB
  decl[declarations 索引]
  scope[scopes 可见性]
  req[requirements 需求与原因]
  bind[binding 名称/成员/调用]
  inf[inference 约束]
  evi[evidence 选择与替换]
  chk[checking 控制流/effect/穷尽性]

  decl --> scope
  req --> bind
  bind --> inf
  inf --> evi
  evi --> req
  bind --> chk
  inf --> chk
```

语义闭包是固定点 **收敛后的结果**，不是提前按名字扫出来的最终名单；发现新实例或 evidence 需求时必须能回到同一作用域与索引事实。

## 当前代码映射（向目标子树收敛）

| 目录/模块 | 目标职责 |
|-----------|----------|
| `closure` | 入口语义闭包规划（`semantic_closure` 迁入） |
| `typing` | 类型线性化、继承分析等 |
| `type_checker` | 语义检查与类型判定入口 |
| `control_flow` | 控制流协议与调度相关语义 |
| `module` | 模块图与声明归属 |
| `operator_members` | 运算符成员名（运算符是普通成员调用） |

尚未单独成目录、仍散落在 `hir` 的能力（overload、type_relation、control_flow_validation 等）归并时应 **消除重复索引**，迁入上述事实所有者，而不是在 `hir/lowering` 再藏一套。

## 边界

- **不** 实现 Nyar 表示或 ABI 具体化（那是 `emitter::frontend_contract` 一类适配）。
- **不** 编排整次编译（`driver`）。
- 错误由本层事实生产者定义种类；统一关联与呈现见 `compiler/diagnostics`。
