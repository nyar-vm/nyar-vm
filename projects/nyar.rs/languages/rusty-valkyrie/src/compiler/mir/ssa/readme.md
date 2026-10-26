# mir ssa

MIR 的 SSA 结构与算法：块参数、值引用、控制流约定集中在此子树。

## Responsibilities

- 承载 SSA 专用的数据结构与变换。
- 为优化与下游 Canonical / 分区提供稳定、目标中立的中间表示。

## Boundaries

- 不混入 `TargetExecutionPlan` 或各目标运行时逻辑。
- 不把 `ssa` 子树做成第二条「万能编译入口」。

## Call 事实从哪来

调用契约来自**已解析的声明**，不由 MIR 或后端重新猜名字：

```text
source → declaration resolve → typed HIR → Semantic MIR → verify → Canonical → representation plan → target-private plan
```

- 声明解析拥有 callable、owner、泛型替换与完整签名。
- 重载选择保留「选中的声明 + 实例身份」，不能用候选名或实例化签名顶替声明身份。
- 静/实例由声明的 `self` 形参决定；Semantic MIR 的普通调用、运算符与集合方法都引用同一套解析结果。
- Canonical 与目标私有计划消费同一身份；公开 ABI 名只在互操作边界出现。
