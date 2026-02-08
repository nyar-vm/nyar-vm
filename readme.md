# Nyar Universal Runtime (NyarVM)

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-success.svg)](#)

**NyarVM** 是一款专为现代编程语言设计的高性能、跨平台虚拟机关编译器基础设施。它通过统一的 **Nyar IR** 层，实现了静态语言与动态语言在同一运行时底座上的无缝协作。

---

## 🏗️ 核心执行架构

NyarVM 的设计核心在于将异构语言前端降解为统一的字节码表示，并根据运行环境灵活选择执行策略。

```mermaid
graph TD
    subgraph Frontends ["语言前端 (Source Layer)"]
        direction LR
        SL[静态语言: C, Java, Valkyrie...]
        DL[动态语言: TS, Python, Lua...]
    end

    subgraph IR_Layer ["中间表示 (Nyar IR)"]
        IR[Nyar Bytecode: 基于栈的高级指令集]
    end

    subgraph Runtime ["统一运行时 (Nyar Runtime)"]
        direction TB
        VM[解释器: 快速启动/调试]
        
        subgraph Acceleration ["加速引擎"]
            JIT[JIT: 基于热点与类型的动态优化]
            AOT[AOT: 静态编译至原生机器码]
        end
        
        subgraph Management ["资源管理"]
            GC[nyar-gc: 分块式 TLAB 垃圾回收]
            Types[nyar-types: NaN-Boxing 类型系统]
        end
    end

    SL -->|Static Lowering| IR
    DL -->|Dynamic Lowering| IR
    IR --> VM
    VM <--> JIT
    VM <--> AOT
    Runtime <--> OS[Windows / Linux / macOS / WASI]

    style IR_Layer fill:#f9f,stroke:#333,stroke-width:2px
    style Runtime fill:#bbf,stroke:#333,stroke-width:1px
```

---

## 🏎️ 混合执行策略

### 1. 静态语言流 (以 C 为例)
静态语言利用 NyarVM 严谨的类型系统，在编译阶段完成单态化与去抽象化。

```mermaid
graph LR
    C_Src[C 源代码] -->|Frontend| IR[Nyar IR]
    IR -->|Static Analysis| Plan{执行决策}
    Plan -->|生产环境| AOT[nyar-aot: 原生二进制]
    Plan -->|开发阶段| JIT[nyar-jit: 即时优化机器码]
    Plan -->|极速预览| Interp[VM: 解释执行]
```

### 2. 动态语言流 (以 TypeScript 为例)
动态语言通过 **NaN-Boxing** 技术实现高效的运行时类型分发。

```mermaid
graph TD
    TS_Src[TS 源代码] -->|Frontend| IR[Nyar IR]
    IR -->|VM| Profile[运行时类型采样]
    Profile -->|收集 Tag| Guard{类型守卫消除}
    Guard -->|Hot Path| JIT[生成特化机器码]
    IR -->|Snapshot| AOT_Snap[预编译二进制快照]
```

#### NaN-Boxing 64-bit 内存布局
所有动态类型（指针、整数、布尔）均压缩在 8 字节字宽内，零开销识别。
```mermaid
packet-beta
title NaN-Boxing Value Layout
0-12: "NaN Base (All 1s)"
13-16: "Tag (4 bits)"
17-63: "Payload (47 bits: GC Ptr / i47 / bool)"
```

---

## 🧩 核心基础设施

### 1. 代数效应 (Algebraic Effects)
Nyar IR 原生支持高阶控制流，通过捕获 **Continuation** 实现极致性能的异步与协程。

```mermaid
sequenceDiagram
    participant Op as Nyar Opcode
    participant VM as VM Runtime
    participant Handler as Effect Handler
    
    Op->>VM: Perform(Effect)
    VM->>VM: Capture Current Stack (Continuation)
    VM->>Handler: Jump to Nearest Handler
    Handler->>VM: Process & Resume
    VM->>Op: Restore Stack & Continue
```

### 2. 内存管理 (nyar-gc)
专为 Nyar 执行模型设计的垃圾回收器，支持线程本地分配缓存 (TLAB)。

```mermaid
graph LR
    subgraph GC_Heap ["分块式堆空间"]
        Block1[Block 0]
        Block2[Block 1]
        BlockN[Block N]
    end
    
    subgraph Threads ["工作线程"]
        T1[Thread 1] -->|TLAB| Block1
        T2[Thread 2] -->|TLAB| Block2
    end
```

---

## 🏗️ 项目模块导航

| 模块 | 路径 | 功能描述 |
| :--- | :--- | :--- |
| **虚拟机核心** | [`nyar-vm`](./projects/nyar-vm) | 字节码解释器、异步运行时及驱动 |
| **类型系统** | [`nyar-types`](./projects/nyar-types) | NaN-Boxing 实现与核心类型定义 |
| **垃圾回收** | [`nyar-gc`](./projects/nyar-gc) | 高性能分块式垃圾回收器 |
| **加速引擎** | [`nyar-jit`](./projects/nyar-jit) / [`nyar-aot`](./projects/nyar-aot) | 动态与静态编译优化工具链 |
| **示例前端** | [`examples/`](./examples) | C, Java, Go, TS 等多种语言的 Nyar 实现 |

---

## 🛠️ 快速开始

```powershell
# 构建全量发布版本
cargo build --release

# 运行基准测试
cargo bench -p nyar-vm

# 执行 Nyar 字节码程序
cargo run -p nyar-vm -- <input_file>
```

## 📖 深入探索
- [Nyar IR 指令集详情](./documentation/zh-hans/maintenance/nyar-ir.md)
- [对象级动态技术](./documentation/zh-hans/guide/dynamic.md)
- [NaN-Boxing 系统深度解析](./documentation/zh-hans/maintenance/nyar-types.md)
