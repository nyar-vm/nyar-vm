# 后端维护指南

本文档介绍了 Valkyrie 编译器支持的各种后端的编译流程和设计考量。

## 概览

Valkyrie 现已采用统一的、以 **Nyar VM** 为核心的编译优化流水线。核心逻辑已从传统的 LIR 线性转换演进为基于 E-Graph 的等价饱和优化：

`Source -> AST -> HIR -> UIR (Chomsky) -> Optimized UIR (Nyar VM) -> Target`

这种架构允许我们将复杂的语言特性优化任务交给专业的优化引擎，而前端只需专注于语义降级。

## 后端实现

- [Nyar VM](nyar-vm.md): **核心后端**。
    - **AOT 模式**: 通过 `NyarAot` 与 `Gaia` 发射静态二进制。
    - **JIT 模式**: 通过 `NyarJit` 实现即时编译与热点优化。
- [WASM 后端](wasi.md): 针对 WebAssembly (WASI)，利用 Nyar VM 的 UIR 发射适配。
- [Native 后端](native.md): 现已完全由 Nyar VM / Gaia 架构接管，提供原生指令集支持。
- [JVM/CLR 后端](jvm.md): 传统后端，针对基于栈的虚拟机，通常跳过 LIR 阶段直接从 CFG/UIR 生成指令。

## 设计决策

### 1. 统一优化入口

自 2026 年起，所有的核心优化任务（包括内联、逃逸分析、死代码消除等）统一由 Nyar VM 驱动的 Chomsky 引擎完成。这避免了在不同后端重复实现优化 Pass 的维护成本。

### 2. 针对栈机跳过寄存器分配

对于 JVM 或 CLR 等栈机后端：
- **理由**：LIR 是为寄存器机设计的。将 SSA 映射到 LIR 涉及寄存器分配，这对于栈机来说是不必要的。
- **策略**：直接从 CFG 或优化后的 UIR 生成基于栈的指令，可以更自然地利用操作数栈。
