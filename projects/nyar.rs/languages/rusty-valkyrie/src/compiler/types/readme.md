# 编译器共享类型

本模块提供源位置、名称路径、诊断和 HIR 数据结构，供语法接入、语义分析、HIR lowering 和 MIR 构造共享。

`SourceID`、`SourceSpan`、`Identifier` 和 `NamePath` 从 `nyar-types` 导入。诊断数据包含报告级别、源码标签及帮助信息；`hir` 描述函数、表达式、声明和语言类型；`witness` 描述 trait 动态分派的方法与关联类型；`advanced` 提供高级类型相关的数据结构。

HIR 模型定义在 [types/hir](hir/readme.md)，分析与构造算法在 [compiler/hir](../hir/readme.md)。MIR 使用独立的模型，详见 [compiler/mir](../mir/readme.md)。
