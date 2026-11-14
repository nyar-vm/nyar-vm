# Nyar

Nyar 提供可组合的编译变换、分析、优化与特化工具，并组织目标代码服务和产物装配。每种语言保留自己的 AST、IR/MIR、语义与解释器。

OdinVM 执行 OVM 目标生成的、经过验证的 `.odin` 模块，可接入 JIT。Wasm、CLR、JVM 与 native 是并列目标，各自拥有目标表示与运行时合同。

LegendVM 提供 guest 执行与 Futamura 特化基底。动态语言以自身解释器为语义依据，特化已知程序并保留未知操作的运行时依赖。当前有限 Lua native residual 路径尚不构成完整的多语言、多目标编译能力。

阅读[架构与当前边界](documentation/architecture.md)、[前端状态](documentation/frontends/index.md)和[目标合同](documentation/backends/index.md)。
