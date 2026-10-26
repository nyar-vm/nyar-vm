# compiler / mir

**Semantic MIR**：带语义与 effect 信息的中间表示，是语言侧最后一层可验证 IR，再交给 `emitter` 做 Nyar 合同适配。

## 职责

- **模型**：函数、块、值、指令、terminator（见子模块与 [`ssa/readme.md`](ssa/readme.md)）。
- **lowering**：从已验证 HIR 构造 MIR；共享 lowering 上下文，但不持有 Legion/Asgard 产品状态。
- **validation**：MIR 阶段合同、与 HIR 一致性、link 前校验。

## 与 HIR / emitter 的边界

```mermaid
flowchart LR
  hir[HirModule]
  mir[MirModule / SSA]
  canon[CanonicalProgram]
  emit[nyar-emitter]

  hir --> mir
  mir --> canon
  canon --> emit
```

- MIR **不** 编码目标指令或寄存器分配；那是 Nyar 后端与目标 lowering 的所有权。
- 布局与单例计划等 **语言仍关心** 的事实可在 MIR 层计算，通过 `emitter` 写入 canonical 载荷。

## 优化

语言语义优化若作用于 MIR，实现放在 [`../optimization/readme.md`](../optimization)；须声明接受的 IR 形态与失效的分析。
