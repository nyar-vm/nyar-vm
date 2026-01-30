# nyar-jit

`nyar-jit` 负责将热点代码段从解释执行转换为本机机器码执行。通过深度集成 `ProjectChomsky` 的优化能力与 `Gaia` 引擎的底层抽象，实现接近原生的执行效率。

## 核心设计哲学：等价饱和优化 (Equality Saturation)

与传统编译器采用固定顺序的优化 Pass 不同，`nyar-jit` 的核心是基于 **E-Graph** 的等价饱和优化：

- **非破坏性变换**: 优化过程不会丢失原始信息，而是不断在 E-Graph 中添加等价的表达式。
- **全局最优提取**: 利用 `UniversalOptimizer` 根据代价模型（Cost Model）从成千上万种等价组合中提取出当前架构下的最优指令序列。
- **跨语言优化**: 由于 `IKun` 意图流是语言无关的，同一套优化逻辑可以同时服务于多种托管语言。

## 编译流水线

`nyar-jit` 的工作流程分为以下六个阶段：

1.  **Hotness Tracking (热点追踪)**: VM 监控字节码执行频率。当函数或循环的调用次数超过预设阈值时，触发 JIT 编译。
2.  **Intent Extraction (意图提取)**: 从运行中的 VM 提取 `IKun` 意图流。这不仅仅是简单的字节码翻译，还包含了运行时的类型信息和分支预测数据。
3.  **E-Graph Saturation (等价饱和)**: 将意图流注入 `UniversalOptimizer`，在 E-Graph 中进行代数化简、公共表达式消除 (CSE) 等优化。
4.  **Instruction Selection (指令选择)**: 根据目标后端的指令集特征，从饱和后的 E-Graph 中提取最优 `IKunTree`。
5.  **Machine Code Generation (机器码生成)**: 调用 `Gaia` 后端将 `IKunTree` 降级为特定架构（如 x86_64, ARM64）的二进制指令。
6.  **Secure Execution (安全执行)**: 将生成的机器码写入 `JitMemory`，应用 W^X 安全策略后跳转执行。

## 现代 JIT 技术挑战与应对

为了应对高性能动态语言的需求，`nyar-jit` 引入了多项前沿技术：

### 1. 推测性优化与去优化 (Speculative Optimization & Deoptimization)
-   **趋势**: 动态语言往往缺乏静态类型。JIT 编译器会根据历史运行数据假设变量类型（推测）。
-   **机制**: 如果推测成功，代码运行极快；如果运行中发现假设失败（如变量类型改变），则触发 **Deoptimization**，安全地回退到解释器状态并重新编译。

### 2. 内联缓存 (Inline Caching)
-   **挑战**: 虚函数调用或动态分发（Dynamic Dispatch）是性能杀手。
-   **优化**: `nyar-jit` 使用单态或多态内联缓存。在机器码中直接嵌入最近使用的目标地址，将间接跳转转化为直接跳转甚至内联。

### 3. 栈上替换 (On-Stack Replacement, OSR)
-   **场景**: 对于一个运行时间极长的死循环，如果仅在函数入口触发 JIT，则该循环永远无法享受到优化。
-   **解决**: OSR 允许在函数运行中途（通常在循环回跳点）将执行权从解释器无缝移交给 JIT 生成的机器码。

### 4. 协程与代数效应优化 (Coroutines & Effects Optimization)

NyarVM 原生支持代数效应（Algebraic Effects），这对 JIT 提出了更高要求。`nyar-jit` 针对这一领域实现了深度优化：

-   **效应内联 (Effect Inlining)**: 通过静态分析或内联缓存（IC）识别特定的 Effect Handler。如果 Handler 是确定的，JIT 会将其逻辑直接内联到 `perform` 调用点，消除动态分发开销。
-   **延续对象的标量替换 (Scalar Replacement of Continuations)**: 对于不逃逸当前作用域的协程延续（Continuation），JIT 会将其状态拆散并分配到寄存器中，避免在堆上创建延续对象。
-   **尾调用优化 (Tail-Call Optimization)**: 许多效应模式（如状态传递）本质上是递归的。`nyar-jit` 确保这些调用被转换为跳转，防止栈溢出并提升性能。
-   **栈图与精确追踪**: 为了支持多跳延续（Multi-shot Continuations），JIT 生成的机器码包含精确的栈图（Stack Maps），确保 GC 在协程挂起和恢复时能准确识别根集合。

## 与 Gaia 引擎的集成

`nyar-jit` 并不直接操作硬件，而是通过 `gaia-jit` 抽象层实现跨平台：

-   **W^X 内存管理**: 严格遵循“不可同时读写与执行”的原则。内存页在写入时为可写不可执行，在执行前切换为只读且可执行。
-   **架构感知**: `Gaia` 提供了丰富的指令描述符，使得 `nyar-jit` 可以利用现代 CPU 的 SIMD、AVX-512 等高级指令特性。

## 开发调试

在调试 JIT 生成的代码时，可以使用以下环境变量：

- `NYAR_LOG_JIT=1`: 打印 JIT 编译过程的详细日志（包括 E-Graph 状态）。
- `NYAR_DUMP_ASM=1`: 将生成的机器码反汇编并保存到本地。
- `NYAR_JIT_THRESHOLD=N`: 手动调整触发 JIT 编译的热点阈值。
