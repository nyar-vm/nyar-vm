# 动态语言执行与编译

Python、Lua 和动态 TypeScript 由语言自己的解释器定义语义。LegendVM 提供 guest 注册、执行调度和特化基底，Nyar 提供部分求值与残余化工具。

第一投影 `R_L,P = S(I_L, P)` 固定解释器和程序，生成接受运行期输入的残余程序。未知属性、动态调用、模块与宿主访问继续由显式 runtime 支持。编译器必须保留这些依赖，不能换成退出码、空值或手工白名单。

残余程序分别适配 Wasm、CLR、JVM、OVM 或 native。OdinVM 只执行 OVM 产物。当前有限 Lua native residual 接线不代表上述目标矩阵已实现。完整边界见[架构合同](../architecture.md)。
