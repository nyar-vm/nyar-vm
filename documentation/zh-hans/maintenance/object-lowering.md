# Valkyrie 编译器：降级指南 (Lowering Guide)

## 1. 降级哲学：从语义到意图

Valkyrie 编译器的核心架构已从传统的“渐进式降级”演进为**意图映射 (Intent Mapping)**。核心目标是将高级语言特性降级为 **ProjectChomsky** 可理解的通用中间表示 (UIR/IKun)。

```mermaid
graph TD
    subgraph Frontend (valkyrie-compiler)
        A[源代码] -->|解析| B(<b>Oaks AST</b>);
        B -->|语义分析| C(<b>HIR</b><br><i>类型, 作用域, Traits</i>);
    end

    subgraph Mid-end (Lowering & Optimization)
        C -->|<b>UIR Lowering</b>| D(<b>Chomsky UIR</b><br><i>意图图, IKun Tree</i>);
        D -->|<b>Equality Saturation</b>| E(<b>Optimized UIR</b><br><i>Nyar VM / Chomsky</i>);
    end

    subgraph Backends (Nyar VM / Gaia)
        E -->|AOT Emission| F[<b>Native Binary</b>];
        E -->|JIT Execution| G[<b>Memory Execution</b>];
        E -->|WASI Export| H[<b>WASM Module</b>];
    end
```

## 2. 现代降级流程 (Modern Lowering)

为了充分利用 Nyar VM 的优化能力，Valkyrie 采用统一的降级路径：

### 2.1 降级到 UIR 的优势

| 特性 | 传统 (SSA/LIR) | 现代 (Chomsky UIR) |
| :--- | :--- | :--- |
| **控制流** | 显式跳转/基本块 | 声明式意图 (If/Loop Intents) |
| **优化时机** | 固定顺序的 Pass | 基于代价模型的全局等价饱和 |
| **后端适配** | 需要为每个后端写代码发射 | 统一由 Gaia 驱动，后端只需定义代价模型 |

## 3. 特性降级示例：模式匹配 (Pattern Matching)

模式匹配是 Valkyrie 的核心特性。我们将追踪它如何降级为 Chomsky 意图。

### 3.1 AST -> HIR
- **模式解析**: 识别嵌套模式和守卫。
- **类型绑定**: 为每个模式分量分配类型。

### 3.2 HIR -> UIR (Lowering)
- **决策意图 (Decision Intents)**: 将 `match` 转化为一系列嵌套的 `Select` 意图。
- **数据流映射**: 将模式中的变量绑定映射为 UIR 中的 `Define` 或 `Bind` 节点。
- **穷尽性检查**: 仍在 HIR 阶段完成，确保生成的 UIR 树是逻辑完整的。

### 3.3 UIR Optimization (Chomsky)
- **分支折叠**: 如果匹配器是常量，Chomsky 会通过等价重写直接消除不必要的分支。
- **等价合并**: 如果多个分支的执行意图相同，它们会在 E-Graph 中被合并。

## 4. 特性降级示例：代数效应 (Algebraic Effects)

Valkyrie 的代数效应通过 **Nyar VM** 的原生效应支持实现。

### 4.1 HIR -> UIR
- **效应降级**: 将 `try/handle` 映射为 UIR 的 `EffectScope` 和 `Handle` 意图。
- **延续捕获**: 将 `resume` 显式化为 UIR 的 `Continuation` 调用。

### 4.2 Nyar VM 执行/生成
- **AOT 模式**: Nyar VM 将效应映射为目标平台的高效状态机实现（如 CPS 变换或轻量级线程）。
- **JIT 模式**: 直接利用 Nyar VM 的原生协程和效应处理器实现，减少上下文切换开销。
