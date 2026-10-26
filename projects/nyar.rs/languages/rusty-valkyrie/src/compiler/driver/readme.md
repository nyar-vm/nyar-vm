# 编译协调

Driver 集中放置编译阶段之间的协调设施，包括 HIR/MIR 验证编排，以及输入修订和失效关系的数据接口。

`validation` 连接 HIR、控制流与 MIR 的一致性检查；`incremental` 定义 `InputRevision` 和 `InvalidationGraph`，表达输入版本及查询间的依赖关系。

项目编译的 pipeline 入口位于 [emitter](../emitter/readme.md)，HIR 构造入口位于 [hir](../hir/readme.md)。理解一条编译请求时，可从产品调用的 pipeline 入口向下追踪，再查看这里协调的验证步骤及各阶段的具体算法。
