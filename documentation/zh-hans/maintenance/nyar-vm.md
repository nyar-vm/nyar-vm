# nyar-vm

`nyar-vm` 是整个 Nyar 生态系统的核心，提供了一个基于操作数栈的高效字节码执行引擎。

## 核心职责

- **字节码规范**: 定义 `Nyarc` 二进制模块格式。
- **执行引擎**: 实现基于栈的解释器。
- **内存管理**: 基于 **Nyar GC**，支持 RAII 语义与析构函数即终结器机制，确保资源安全。
- **异步支持**: 原生协程与 `Await` 机制。
- **代数效应 (Effect System)**: 提供比异常处理更强大的控制流抽象。

## 模块格式 (Nyarc)

`.nyar` 文件由以下部分组成：
- **Header**: `NYAR` 魔数、版本号、标志位、时间戳。
- **Constant Pool**: 存储整数、浮点数、字符串常量。
- **Chunks**: 包含代码字节、局部变量数、最大栈深度、上值信息。
- **Metadata**: 包含类（Classes）、接口（Traits）、实现（Impls）、导入（Imports）与导出（Exports）信息。

## 核心指令集概览

| 类别 | 指令示例 | 描述 |
| :--- | :--- | :--- |
| **栈操作** | `Push`, `Pop`, `Dup`, `Swap` | 基础操作数栈管理 |
| **变量存取** | `LoadLocal`, `StoreGlobal`, `LoadUpvalue` | 处理不同作用域的变量 |
| **控制流** | `Jump`, `JumpIfFalse`, `Call`, `Return` | 改变执行路径 |
| **面向对象** | `NewObject`, `GetField`, `InvokeMethod` | 类与实例操作 |
| **扩展类型** | `I32Ext`, `F64Ext`, `StringExt` | 针对特定类型的优化指令 |
| **代数效应** | `Perform`, `WithHandler`, `ResumeWith` | Effect 的触发与处理 |
| **元编程** | `Quote`, `Splice`, `Eval` | 运行时代码生成与求值 |

## 解释器循环

解释器通过一个高效的 `match` 循环（或在 JIT 模式下生成的跳转表）执行指令：

```rust
// 伪代码示例
loop {
    let opcode = self.fetch_u8();
    match opcode {
        Opcode::Push => {
            let idx = self.fetch_u16();
            let val = self.module.constants[idx].clone();
            self.stack.push(val);
        }
        // ... 其他指令
        Opcode::Halt => break,
    }
}
```

## Effect 系统工作原理

Effect 系统允许开发者定义抽象的副作用（如 `IO`, `Log`, `State`），并在调用栈的高层进行统一处理。
1.  **Perform**: 触发一个效应，挂起当前上下文。
2.  **WithHandler**: 建立一个处理器上下文。
3.  **ResumeWith**: 处理器处理完后，将结果返回给被挂起的上下文并继续执行。
