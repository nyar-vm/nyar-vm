# MIR SSA

SSA 子模块提供 Semantic MIR 的函数、基本块、值引用、指令和终结指令，并实现 HIR 到这些结构的转换。块参数描述控制流合流时传入的值，调用合同携带已解析声明与实例信息。

```mermaid
flowchart LR
  hir[HIR 函数] --> lowering[语句与表达式 lowering]
  lowering --> blocks[基本块与 SSA 值]
  blocks --> validation[MIR validation]
  validation --> emitter[编译适配]
```

阅读 lowering 时，可沿函数上下文、语句转换、表达式转换和 terminator 构造追踪值的产生与流向。类型、聚合字段和调用身份随 MIR 进入模块验证与后续编译适配。
