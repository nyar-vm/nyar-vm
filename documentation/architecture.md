# 编译与执行架构

本文是本仓唯一的跨语言、跨目标架构合同。目录和 API 的当前存在，不代表已经满足该合同。

## 语言拥有语义

Valkyrie、TypeScript、Python、Lua 各自拥有语法、名字解析、类型与对象模型、控制流、AST、语言 IR/MIR、解释器及语义验证。语言可以选择不同的内部表示和编译策略，不必经过同一个 MIR。

现有 `CanonicalProgram`、`CanonicalSemanticMir`、`CanonicalOperation` 和 `CompiledProgram` 承载 Valkyrie 语义，应重命名并归还 Valkyrie。它们目前仍出现在公共编译链中，这是待移除的架构债务，不能重新包装成可选共享语义子集。

## Nyar 提供工具

Nyar 提供可组合的变换、分析、优化、部分求值与残余化工具，以及 pass 编排、中性身份、链接、分区、能力和产物合同。语言为所使用的工具提供操作接口、合法性依据和成本模型。对象代数、E-Graph 或某种分析结果可以按需使用，不是所有语言的强制中间层。

Nyar 不拥有跨语言的语义总 IR，不将 Valkyrie 的复杂控制流升格成其他语言的必经协议，不通过名字猜测语义，不在 emitter 补齐语言事实。

## 动态语言与 Futamura

Python、Lua 与动态 TypeScript 的编译必须明确保留其解释器语义。第一投影写作 `R_L,P = S(I_L, P)`：已知语言解释器 `I_L` 与程序 `P`，由特化器 `S` 生成残余程序 `R_L,P`，对运行期输入 `x` 满足 `R_L,P(x) ≃ I_L(P, x)`。

已知事实可被求值和优化。运行期属性、动态调用、模块、反射、异常与宿主访问中尚未知的部分，必须保留为残余操作和显式运行时依赖。不能用编译时执行一次程序、折叠退出码或返回占位值替代编译。

LegendVM 是 guest 执行与特化基底，负责注册、调度及特化接缝。具体语言解释器留在语言 crate，LegendVM 不复制完整 tree-walk，也不要求基于 OdinVM 实现。第二、第三投影属于后续方向，不能宣称已经交付。

## 目标拥有表示与执行合同

语言到目标的适配器负责把本语言 IR 或残余程序降低到目标拥有的计划和 LIR。Wasm、CLR、JVM、OVM、native 各自拥有表示、调用约定、验证、运行时包装和产物规则。同一目标代码服务可供不同语言使用，不要求语言共享 MIR。

- Wasm：模块、线性内存或 GC 表示及宿主边界。
- CLR：CLI 类型、MSIL、程序集与托管运行时包装。
- JVM：类、方法、字节码及 Java 运行时包装。
- OVM：OVM 计划和指令，编码为 `.odin`，由 OdinVM 执行。
- Native：机器码、对象文件、重定位、链接和平台容器服务。

OdinVM 加载和执行经过验证的 `.odin`，可以注入 JIT。它是一个目标运行时，不是所有后端的公共执行层。Wasm、CLR、JVM 和 native 不绕道 `.odin`。

Native 的 `.node` 必须满足 Node-API 导出、初始化与生命周期合同，`.pyd` 或 Python `.so` 必须满足 CPython 扩展初始化、引用所有权与异常合同。文件后缀不能创造 ABI。编译器自身的 N-API 绑定也不等于 guest 程序的扩展编译 lane。

## 当前实现边界

当前 Cargo workspace 的语言成员为 `rusty-valkyrie` 与 `rusty-{bash,c,lua,powershell,tcl}`。Python、TypeScript 等目录不在活动 workspace 中，不能从目录存在推导支持已交付。

现有编译链仍有 Valkyrie 专用 canonical/executable 类型和跨目标共用计划的迁移债务。Lua 已接有限的 native residual 路径，但完整逐操作 native 代码生成尚未完成，退出码折叠或 stub 不构成动态语言编译交付。各 target 的已有实现也不代表每种语言都能使用。

验收按语言语义 → 工具变换合法性 → 目标适配与运行时 → 真实产物逐层进行。测试通过不能覆盖层次错误，手填中间合同不能替代源码到产物证据。
