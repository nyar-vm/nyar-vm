# Valkyrie 虚拟机维护指南

本指南面向 Valkyrie 虚拟机项目的**内部维护者和核心开发团队**，介绍项目架构、模块职责、内部维护流程和系统级设计决策。

> **目标读者**: 项目维护者、核心开发团队成员、系统架构师
> **内容重点**: 内部架构、维护流程、系统设计、代码组织

## 项目架构概览

Valkyrie 采用 Rust Monorepo (Workspace) 架构，每个组件封装在独立的 crate 中，提供清晰的依赖关系、独立的测试环境和高效的并行编译。

```
valkyrie/
├── Cargo.toml         # Workspace 根配置
└── projects/
    ├── valkyrie-types/    # 统一中間表示類型定義 (HIR, CFG, SSA, LIR)
    ├── valkyrie-compiler/ # 基於 Chomsky 的現代編譯器框架
    ├── valkyrie-runtime/  # 編譯器遍 (Pass) 與虛擬機執行核心
    ├── valkyrie-error/    # 基於 miette 的診斷系統
    └── legion/            # 命令行工具 (Valkyrie 工具鏈入口)
```

外部依賴:
- `oak-valkyrie`: 新版前端實現 (Lexer, Parser, AST)，位於 `../oaks`
- `ProjectChomsky`: 編譯器後端優化框架，位於 `../ProjectChomsky`

## 核心设计哲学

Valkyrie 的架构基于五大设计支柱：

### 1. 现代编译流水线 (Modern Compilation Pipeline)

Valkyrie 的核心设计已演进为以 **Nyar VM** 为中心的现代化架构，将优化与后端生成任务完全解耦：

#### 阶段 1: 前端 (Frontend - Oaks)
- **职责**: 语法解析与高级语义处理。
- **当前实现**: 使用 `oak-valkyrie` 作为统一的前端。
- **关键处理**:
  - **符号解析 (Symbol Resolution)**: 构建跨模块的符号引用。
  - **类型检查与推导 (Type Checking & Inference)**: 确保语言层面的类型安全。
  - **模式匹配解糖 (Pattern Matching Desugaring)**: 将复杂的 `match` 结构转换为决策树。

#### 阶段 2: 降级 (Lowering - Chomsky UIR)
- **职责**: 将高级语义 (HIR) 转换为通用的、可优化的中间表示 (UIR/IKun)。
- **关键处理**: 将语言特定的语义原语映射到通用的 UIR 意图（Intents）。

#### 阶段 3: 优化 (Optimization - Nyar VM / Chomsky)
- **职责**: 执行全局优化，支持 AOT 和 JIT。
- **核心技术**: **E-Graph 等价饱和**。
- **优势**: 统一的优化逻辑，无需为不同后端重复编写优化 Pass。

#### 阶段 4: 后端发射 (Backend Emission - Nyar VM / Gaia)
- **职责**: 针对特定目标（WASM, Native, JIT 内存空间）发射代码。
- **关键处理**: 寄存器分配、指令选择。
  - **栈帧优化**: 减少不必要的入栈出栈。
  - **指令调度**: 优化执行流水线以减少延迟。

### 2. 开发者体验的终极追求 (Uncompromising Developer Experience)

- **诊断即对话**: 使用 `miette` 框架提供 IDE 级别的诊断体验
- **心流不被打断**: 通过高效的编译流水线实现亚秒级响应
- **直觉且强大的语言**: 提供代数效应、强大的模式匹配等高级抽象

### 3. 抽象的统一与对称 (Unity and Duality of Abstractions)

基于数据与控制的对偶性：
- `match` 表达式：对数据的分解和模式匹配
- `handle` 表达式：对控制流（代数效应）的分解和模式匹配

### 4. 执行模型的二元性 (Duality of Execution Models)

- **动态解释执行**: 专为开发、调试和交互式环境设计，内建完整运行时
- **静态编译执行**: 专为生产部署设计，编译为轻量、高效的 WebAssembly 模块

### 5. 零成本抽象的最终承诺 (Zero-Cost Abstraction)

高级抽象在编译后应与手写的最优底层代码同样高效。

## 核心模块详解

### oak-valkyrie: 编译器前端实现
**职责**: 提供 Lexer, Parser 和 AST 定义，将源文本解析为抽象语法树。

### valkyrie-types: 中间表示类型定义
**职责**: 集中管理 HIR, CFG, SSA, LIR 等各阶段的中间表示类型定义。

### valkyrie-vm: 虚拟机与编译器核心
**职责**: 实现各阶段之间的转换 (Pass) 与最终执行。

### valkyrie-error: 统一错误处理
**职责**: 提供集中的错误定义和诊断信息输出。

## 维护流程

### 代码审查标准
1. **架构一致性**: 确保新代码符合五大设计支柱
2. **错误处理**: 使用统一的 `valkyrie-error` 系统
3. **性能考虑**: 避免不必要的分配和拷贝

### 调试指南
1. **编译器错误**: 检查 `valkyrie-error` 的诊断输出
2. **代码生成问题**: 使用 `--dump-{ast,hir,cfg,ssa,lir}` 选项转储中间层输出

---

## 设计与实现专题

- [项目架构设计](project-architecture.md)
- [执行模型 (解释与编译)](execution-models.md)
- [对象降低 (Object Lowering)](object-lowering.md)
- [包管理与符号解析](package-management.md)
- [基于 Miette 的错误处理](error-handling.md)
- [性能优化策略](optimization-strategies.md)
- [后端实现与考量](backends/index.md)

---
本维护指南将随着项目的发展持续更新。
