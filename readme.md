# Nyar VM

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-success.svg)](#)

## 💡 什么是 nyar-vm？

**Nyar VM** 是一套现代化的高性能运行时，主要包括两种场景：

- **OdinVM**：高性能的栈式 VM，加载 `.odin` 字节码，支持并发式无停顿 GC 和自适应 Jit 加速。
- **LegendVM**：在 OdinVM 基础上构建的解释型运行时，可通过 Futamura 映射生成独立二进制分发。

两个运行时都以语言中立为前提：语言语义属于各自的前端，运行时只消费已经闭合的输入。

## 🏗️ 整体架构

```mermaid
graph TD
    classDef src fill:#e3f2fd,stroke:#2196f3,stroke-width:2px;
    classDef platform fill:#fff3e0,stroke:#ff9800,stroke-width:2px;
    classDef vm fill:#f3e5f5,stroke:#9c27b0,stroke-width:2px;
    classDef engine fill:#e8f5e9,stroke:#4caf50,stroke-width:2px;

    subgraph Frontends ["语言前端"]
        direction LR
        V["Valkyrie 前端"]:::src
        H["Bash / Lua / PowerShell / Tcl / C"]:::src
    end

    subgraph Platform ["Nyar 编译平台"]
        PLAN["规划与降低 (nyar)"]:::platform
        EMIT["产物装配 (nyar-emitter)"]:::platform
    end

    subgraph Runtimes ["nyar-vm 运行时"]
        direction LR
        subgraph Odin ["OdinVM"]
            direction TB
            BYTECODE["odin-bytecode (.nyar)"]:::vm
            OVM["odin-vm 解释器"]:::vm
            JIT["nyar-jit 即时编译"]:::engine
            GC["nyar-gc 托管堆"]:::vm
        end
        subgraph Legend ["LegendVM"]
            direction TB
            GUEST["guest 注册与分发"]:::vm
            INTERP["语言解释器"]:::vm
            PE["Futamura 特化"]:::vm
            NATIVE["原生残余 (PE / x64)"]:::engine
        end
    end

    AOT["AOT 预编译产物"]:::engine

    V --> PLAN
    PLAN --> EMIT
    EMIT -->|OVM 后端| BYTECODE
    EMIT -->|AOT| AOT
    BYTECODE --> OVM
    OVM <--> JIT
    OVM --- GC
    H --> GUEST
    GUEST --> INTERP
    INTERP -->|build --target native| PE
    PE --> NATIVE
```

---

## ⚙️ OdinVM

OdinVM 执行 OVM 后端的产出：前端语义闭合后，经 Nyar 平台降低为 `nyar-ir`，再由 `nyar-emitter` 装配成 `odin-bytecode` 模块。运行时只看到已编码、已验证的模块，不接触任何语言的 AST 或解析器。

### 执行模型

```mermaid
graph LR
    classDef core fill:#f3e5f5,stroke:#9c27b0,stroke-width:2px;
    classDef engine fill:#e8f5e9,stroke:#4caf50,stroke-width:2px;

    Module["odin-bytecode (.nyar)"] --> Loader["加载与校验"]
    Loader --> Exec["解释器 (Executor)"]:::core
    Exec <--> JIT["nyar-jit"]:::engine
    Exec --- GC["nyar-gc"]:::core
    Exec <--> Host["宿主导入 (HostOp)"]
```

### 核心特性

- **字节码解释**：栈式指令循环、调用帧、模块初始化函数与全局槽。模块加载时校验结构、解析导出符号。
- **宿主导入**：通过 `CallImport` 与 `host` 模块的 `HostOp` 派发调用宿主能力。
- **代数效应**：`PerformEffect` 挂起当前上下文，交由最近的处理器恢复。
- **异步与协程**：`Yield` / `Resume` 在托管堆上保存和恢复协程状态。
- **即时编译**：`nyar-jit` 通过 `Executor::with_jit` 注入，按栈图编译热点函数，假设失效时去优化回解释器。
- **标量快路径**：NJ1 小规模标量程序可直接编译执行，并在解释器内缓存编译结果。
- **内存管理**：`nyar-gc` 提供托管堆、分代标记-清除回收与宿主根。
- **AOT**：编译期的 `nyar-emitter` 直接把程序编译为目标产物；运行时侧对应的是 JIT 的即时优化。

#### 代数效应

```mermaid
sequenceDiagram
    autonumber
    participant Op as OVM Opcode
    participant VM as OdinVM
    participant Handler as Effect Handler

    Op->>+VM: PerformEffect
    Note over VM: 捕获当前栈（Continuation）
    VM->>+Handler: 跳到最近的处理器
    Handler->>-VM: 处理并恢复
    VM->>-Op: 恢复栈并继续执行
```

#### 内存管理

```mermaid
graph LR
    classDef heap fill:#e1f5fe,stroke:#01579b;
    classDef gc fill:#f1f8e9,stroke:#33691e;
    classDef free fill:#ffffff,stroke:#0288d1,stroke-width:2px;

    Roots["栈 / globals / host roots"]:::gc --> Mark["标记 (GcRoots)"]:::gc
    Alloc["分配 (free list)"]:::free --> Nursery["Nursery 年轻代"]:::heap
    Nursery -->|晋升| Tenured["Tenured 老年代"]:::heap
    Mark --> Nursery
    Mark --> Tenured
    Mark -->|清除| Free["回收槽位 → free list"]:::free
    Free --> Alloc
```

---

## 🧩 LegendVM

LegendVM 是一个语言无关的 PE / Futamura 解释基底，结构类似 Truffle：它不理解任何宿主语言的语法，只提供 **guest 注册**、**解释分发** 和 **特化到原生** 三条接缝。

每个宿主语言在自己的 `rusty-<lang>` crate 中保留 AST、语义桥和解释器；LegendVM 侧只有薄适配层，把 guest 值转成统一表示并转交调用。

```mermaid
graph LR
    classDef guest fill:#e3f2fd,stroke:#2196f3,stroke-width:2px;
    classDef vm fill:#f3e5f5,stroke:#9c27b0,stroke-width:2px;
    classDef engine fill:#e8f5e9,stroke:#4caf50,stroke-width:2px;

    Guests["Bash / Lua / PowerShell / Tcl / C"]:::guest --> Seam["guest 注册 (GuestInterpretFn)"]
    Seam --> Runner["LegendVmRunner 分发"]:::vm
    Runner -->|run| Interpret["解释执行"]:::vm
    Runner -->|build --target native| Specialize["Futamura 第一投影"]:::vm
    Specialize --> Residual["NativeResidualModule"]:::engine
    Residual --> PE["Windows PE (.exe)"]:::engine
```

### 核心特性

- **插件式 guest**：新增语言只需要实现自己的解析与解释，再注册一个薄适配器，PE 核心不随之改动。
- **统一入口**：`LegendVmRunner` 按语言 id 选择 guest，解释器是该语言的语义真相源。
- **特化而非重写**：`build --target native`（别名 `pe` / `exe`）把解释器相对于程序做 Futamura 第一投影，得到原生残余后再封装为 PE。
- **职责分离**：`.nyar` 与 OdinVM 只作为探针路径使用，宿主脚本的原生产物走 LegendVM 自己的特化链。

当前 Lua 的解释器特化已接到原生残余出口；其余宿主语言的特化路径仍在完善。

---

## 🏎️ 两条执行路径

### 静态语言

静态语言在前端完成语义闭合，交给 Nyar 平台规划与降低，再按目标选择预编译、即时编译或解释执行。

```mermaid
graph LR
    classDef src fill:#e3f2fd,stroke:#2196f3;
    classDef platform fill:#fff3e0,stroke:#ff9800;
    classDef engine fill:#e8f5e9,stroke:#4caf50;

    Src["Valkyrie 源代码"]:::src --> Frontend["前端：语义闭合"]
    Frontend --> Platform["Nyar 规划与降低"]:::platform
    Platform --> Mode{"执行方式"}
    Mode -->|AOT| AOT["预编译产物"]:::engine
    Mode -->|JIT| JIT["nyar-jit 即时编译"]:::engine
    Mode -->|解释| VM["OdinVM"]:::platform
```

### 动态语言

宿主脚本语言保留自己的解释器，由 LegendVM 分发执行；需要原生产物时再对解释器做特化。

```mermaid
graph LR
    classDef src fill:#fce4ec,stroke:#f06292;
    classDef vm fill:#f3e5f5,stroke:#9c27b0;
    classDef engine fill:#e8f5e9,stroke:#4caf50;

    Src["Bash / Lua / PowerShell / Tcl / C 源代码"]:::src --> Guest["语言自身 AST 与解释器"]
    Guest --> Legend["LegendVM 注册与分发"]:::vm
    Legend -->|run| Interp["解释执行"]:::vm
    Legend -->|build --target native| PE["Futamura 特化"]:::vm
    PE --> Native["原生 PE 产物"]:::engine
```

---

## 📖 深入探索

- [开发者文档](./documentation/)
- [OdinVM](./projects/nyar.rs/runtimes/odin-vm/readme.md)
- [LegendVM](./projects/nyar.rs/runtimes/legend-vm/readme.md)
- [nyar-gc](./projects/nyar.rs/runtimes/nyar-gc/readme.md)
- [nyar-jit](./projects/nyar.rs/runtimes/nyar-jit/readme.md)
- [odin-bytecode](./projects/nyar.rs/formats/odin-bytecode/readme.md)
