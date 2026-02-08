# Nyar Runtime (NyarVM)

[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Status](https://img.shields.io/badge/status-active-success.svg)](#)

**NyarVM** 是一款专为现代编程语言设计的高性能、跨平台虚拟机关编译器基础设施。作为 **Valkyrie** 编程语言的核心执行引擎，它提供了一套统一的运行时环境，旨在打破语言壁垒，实现异构语言在同一高性能底座上的无缝协作。

## 🚀 核心特性

- **代数效应系统 (Algebraic Effects)**: 原生支持高阶控制流抽象，为错误处理、协程、生成器及依赖注入提供强力支持。
- **先进的内存管理 (`nyar-gc`)**: 专为高性能设计的垃圾回收器，支持 TLAB (线程本地分配缓存) 与分块管理，兼顾吞吐量与低延迟。
- **混合执行模型**:
  - **解释执行**: 提供极致的启动速度，适用于开发调试与脚本场景。
  - **JIT (即时编译)**: 针对热点代码进行动态优化，提升长时运行性能。
  - **AOT (预编译)**: 支持静态编译为原生二进制，确保最佳性能与最小的分发体积。
- **异步原生运行时**: 深度集成异步操作与效应系统，提供高性能的非阻塞 I/O 与并发模型。
- **多前端生态**: 统一的中间表示 (IR) 允许 C、Java、Go、Python 等多种语言前端共享优化的后端基础设施。
- **可扩展 FFI**: 稳健的外部函数接口，支持与原生库、文件系统及网络协议栈的高效交互。

## 🏗️ 项目结构

本工作空间由多个核心子项目及语言前端示例组成：

### 核心项目 (`/projects`)

- **[nyar-vm](./projects/nyar-vm)**: 虚拟机核心，包含字节码解释器、驱动程序及异步运行时。
- **[nyar-types](./projects/nyar-types)**: 贯穿整个生态系统的基础类型系统定义。
- **[nyar-gc](./projects/nyar-gc)**: 针对 Nyar 执行模型优化的自定义垃圾回收器。
- **[nyar-jit](./projects/nyar-jit)**: 动态性能优化的即时编译基础设施。
- **[nyar-aot](./projects/nyar-aot)**: 用于生成原生二进制的预编译工具链。
- **[nyar-tools](./projects/nyar-tools)**: 包含编译、运行、调试及基准测试在内的命令行工具集。

### 语言前端 (`/examples`)

NyarVM 提供了多种语言的实验性前端（通常被称为 "Rusty" 系列）：

- **Rusty-C**: 针对 NyarVM 的 C 语言前端实现。
- **Rusty-Java**: 支持在 Nyar 上执行 JVM 兼容字节码。
- **Rusty-Go**: 融合 Go 语言特性的前端实现。
- **Rusty-CSharp**: 提供类似 .NET 能力的 C# 前端。
- **更多支持**: 包括 Python、TypeScript、Swift 及 Julia 在内的前端正在积极开发中。

## 🛠️ 快速开始

### 环境准备

- [Rust](https://www.rust-lang.org/tools/install) (Stable 或 Nightly)
- Cargo 包管理器

### 构建项目

```powershell
cargo build --release
```

### 运行测试

```powershell
cargo test
```

### 运行程序

```powershell
cargo run -p nyar-vm -- <input_file>
```

## 📖 文档中心

关于 Valkyrie 语言及 Nyar 运行时的详细文档请参阅 `documentation/` 目录：

- **[简体中文](./documentation/zh-hans/index.md)**
- **[English](./documentation/en-us/index.md)**
