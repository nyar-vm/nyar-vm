# nyar-types

`nyar-types` 提供了整个工作区共享的基础基础设施，确保各模块之间通信的一致性。

## 核心职责

- **VmError**: 定义了从编译到运行的所有错误类型。
- **FormatError**: 字节码序列化与反序列化错误。
- **Shared Traits**: 提供跨项目使用的通用接口。

## 关键数据结构

### VmError

`VmError` 是系统的核心异常类型，涵盖了：
- `InvalidOpcode`: 遇到无法识别的指令。
- `StackUnderflow`: 操作数栈深度不足。
- `UnhandledEffect(String)`: 触发了效应但没有对应的处理器。
- `RuntimeError(String)`: 通用的运行时异常。

### DecodeError

专门用于 `nyar-vm` 字节码解码阶段的错误处理。

### AOT 错误类型

- `WasmAotError`: 针对 WebAssembly 编译目标的特定错误。
- `JvmAotError`: 针对 Java 虚拟机编译目标的特定错误。

## 贡献指南

在向 `nyar-types` 添加新类型时，请确保：
1.  **最小化依赖**: 该项目被所有其他项目引用，应保持极简。
2.  **兼容性**: 避免破坏性的枚举项更改。
