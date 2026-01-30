# Nyar VM 维护指南

Nyar VM 是 Valkyrie 的主要运行时和优化后端，支持 AOT (Ahead-of-Time) 编译和 JIT (Just-in-Time) 执行模式。

## 核心编译流水线

Valkyrie 编译器在完成前端语义分析后，将控制权移交给 Nyar VM 进行优化和生成：

`Source -> AST -> HIR -> UIR (Chomsky) -> Optimized UIR -> Backend Emission`

### 1. HIR -> UIR (Lowering)
- **职责**: 将语言特定的高级语义（如 Valkyrie 的 Effect System, Pattern Matching）映射到 Chomsky 的通用中间表示 (UIR/IKun)。
- **实现**: [valkyrie-compiler](file:///e:/普遍优化/valkyrie.rs/projects/valkyrie-compiler/src/pipeline/mod.rs) 中的 `lower_root_to_uir`。

### 2. UIR Optimization (Chomsky)
- **职责**: 核心优化阶段。
- **技术**: 使用基于 E-Graph 的等价饱和技术。
- **实现**: [ProjectChomsky](file:///e:/普遍优化/ProjectChomsky) 中的 `UniversalOptimizer`。
- **优势**: 
    - 无论是 AOT 还是 JIT，共享同一套优化逻辑。
    - 极强的全局优化能力（内联、死代码消除、常数传播等）。

### 3. AOT 模式
- **职责**: 静态生成可执行二进制文件或 WASM 模块。
- **流程**: 使用 `NyarAot` 驱动优化流程，并通过 `Gaia` 汇编器生成目标文件。
- **实现**: [nyar-aot](file:///e:/普遍优化/nyar-vm/projects/nyar-aot/src/lib.rs)。

### 4. JIT 模式
- **职责**: 在运行时根据热点代码生成并执行机器码。
- **流程**: 利用 `gaia-jit` 动态发射指令到内存并执行。
- **实现**: [nyar-jit](file:///e:/普遍优化/nyar-vm/projects/nyar-jit/src/lib.rs)。

## 与 ProjectChomsky 的集成

Nyar VM 通过 `IKun` 接口与 Chomsky 深度集成。编译器生成的意图（Intents）被送入 Chomsky 的 E-Graph 中，经过规则重写达到等价饱和状态后，提取出代价最小的树结构用于后端生成。

## 维护建议

- **优化规则**: 如果需要添加新的优化逻辑，应在 `ProjectChomsky` 中添加重写规则。
- **后端适配**: 如果需要支持新的指令集或平台，应在 `project-gaia` 中添加新的汇编器适配。
