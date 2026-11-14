# Native

本目标负责机器码、对象文件、重定位、链接与平台容器。`.node` 需要 Node-API 初始化、导出和生命周期，`.pyd` / Python `.so` 需要 CPython 初始化、引用所有权与异常桥接。后缀不等于 ABI，宿主编译器 N-API 绑定不等于 guest 扩展 lane。

语言到目标适配器消费该语言自己的 IR 或特化后的残余程序，生成目标拥有的计划和 LIR。目标可以提供 runtime wrapper 承载语言功能，但不得解析语言 AST 或在编码器中恢复缺失语义。

其他目标不以本目标的 LIR 或运行时为必经中间层。当前代码中的共用 semantic/executable 合同仍需按[架构合同](../architecture.md)拆回语言和目标；本页描述职责，不宣称迁移或全 lane 能力已经完成。
