# Wasm

本目标负责Wasm 模块、内存或 GC 表示、导入导出与宿主胶水。WASI、浏览器和 JS glue 的宿主合同分别验证。

语言到目标适配器消费该语言自己的 IR 或特化后的残余程序，生成目标拥有的计划和 LIR。目标可以提供 runtime wrapper 承载语言功能，但不得解析语言 AST 或在编码器中恢复缺失语义。

其他目标不以本目标的 LIR 或运行时为必经中间层。当前代码中的共用 semantic/executable 合同仍需按[架构合同](../architecture.md)拆回语言和目标；本页描述职责，不宣称迁移或全 lane 能力已经完成。
