# 语言前端

前端拥有语言语义与自己的 AST/IR/MIR。接入 Nyar 不要求生成 Valkyrie MIR，也不要求所有语言变成共享对象模型。详见[架构合同](../architecture.md)。

活动 workspace 包含 Valkyrie 与 Bash、C、Lua、PowerShell、Tcl guest。guest 解释与编译是不同能力，必须分别验收。

- [Lua](rusty-lua.md)：活动 guest，有限 native residual 路径。
- [Python](rusty-python.md)：非活动 workspace，动态语言目标合同。
- [TypeScript](rusty-typescript.md)：非活动 workspace，静态与动态区域的语言责任。

其他 `rusty-*` 目录保留实验代码，不构成产品支持矩阵。具体源码状态见对应 crate 的 readme 与 Cargo workspace，不能把过时示例 API 当成现行入口。
