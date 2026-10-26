# 编译器诊断

本模块提供 `ReportKind` 与 `ValkyrieError` 的公共访问入口。调用方使用这些类型表达诊断级别、源码范围、标签和关联信息。

具体错误在语法、HIR、MIR 等阶段产生，共享诊断载体定义在 [types](../types/readme.md)。面向宿主的诊断投影与文本展示位于 [utilities](../../utilities/readme.md)。沿这条链路可以从诊断追踪到产生它的编译阶段。
