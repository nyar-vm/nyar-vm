# Valkyrie 执行模型

## 1. 概述

Valkyrie 旨在支持多种执行环境，从高性能的生产环境到高度交互的开发环境。核心执行逻辑由 **Nyar VM** 驱动。

## 2. 现代执行架构 (Modern Execution Architecture)

Valkyrie 采用**统一意图后端架构 (Unified Intent Backend Architecture)**。所有代码首先降级为 Chomsky UIR，然后由 Nyar VM 根据执行模式进行分发：

```mermaid
graph TD
    A[Chomsky UIR] -->|Nyar VM| B{执行模式};
    B -->|JIT| C[动态生成机器码并执行];
    B -->|AOT| D[生成静态二进制文件];
    B -->|Interpreter| E[基于意图的解释执行 (调试用)];
```

### 2.1 JIT 模式 (Just-In-Time)
- **场景**: 开发调试、高性能脚本执行。
- **机制**: Nyar VM 实时分析 UIR 热点，调用 `Gaia JIT` 引擎将 UIR 意图直接发射到内存并执行。
- **优势**: 
    - 结合了动态语言的灵活性和原生代码的高性能。
    - 支持热重载 (Hot Reloading)。

### 2.2 AOT 模式 (Ahead-Of-Time)
- **场景**: 生产环境部署、WASM 模块分发。
- **机制**: 使用 `NyarAot` 静态扫描整个 UIR 意图树，应用深度全局优化后，通过 `Gaia` 发射为目标平台的机器码或字节码（如 WASI, x86_64）。
- **优势**: 
    - 零启动开销。
    - 极致的二进制体积优化。

## 3. Nyar VM 核心特性

无论采用何种执行模式，Valkyrie 都共享由 Nyar VM 提供的运行时能力：

- **基于 E-Graph 的全局优化**: 所有的 AOT 和 JIT 优化均由内置的 Chomsky 引擎驱动。
- **原生代数效应支持**: Nyar VM 在底层实现了高效的效应处理器和延续 (Continuation) 捕获。
- **统一代价模型**: 开发者只需定义一套后端代价模型，即可同时受益于 AOT 和 JIT 的优化。

## 4. 废弃的 LIR/SSA 模型 (Legacy Models)

早期的 Valkyrie 曾计划使用多层线性降级（SSA -> LIR），但为了实现更深度的全局优化，现已全面转向 **Chomsky UIR + Nyar VM** 架构。

- **为何废弃**: 
    - 传统的 SSA/LIR 优化 Pass 顺序固定，难以发现跨阶段的等价优化机会。
    - 维护多套后端发射逻辑（WASM, Native, VM）成本过高。
    - Nyar VM 的等价饱和技术提供了更强的优化上限。
