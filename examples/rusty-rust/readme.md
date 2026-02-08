# Mini Rust 语言编译器

这是一个简化的 Rust 风格语言编译器，主要用于验证 Gaia 项目在处理类 Rust 语法并编译到 .NET CLR (Common Language Runtime) 等托管环境时的正确性。

## 核心目标

- **CLR 验证**：重点验证从类 Rust 代码到 .NET IL (Intermediate Language) 及 CLR 二进制文件 (.dll/.exe) 的转换。
- **多目标支持**：除了 CLR，还支持编译到 JVM、PE、WASI 等多种目标。

## 支持特性

### 语言特性
- **变量管理**：支持 `let` 声明，区分可变 (`mut`) 与不可变变量。
- **数据结构**：支持 `struct` 定义及实例化，支持数组字面量。
- **控制流**：支持 `if-else`、`while` 以及基于范围的 `for` 循环 (`for i in 0..10`)。
- **表达式**：
  - 基础算术与逻辑运算。
  - 函数调用、方法调用、宏调用（如 `println!`）。
  - 字段访问与索引访问。
- **类型系统**：内置 `i32`, `i64`, `f32`, `f64`, `string`, `bool` 等类型。

### 编译目标
可以通过 `--target` 参数指定以下目标：
- `il`: .NET IL (中间语言)
- `clr`: .NET CLR 二进制文件
- `jvm`: Java 虚拟机字节码
- `pe`: Windows 可执行文件
- `wasi`: WebAssembly System Interface

## 快速开始

### 使用方法
```bash
# 编译 Mini Rust 源文件
cargo run -- <input.vrs> --target clr
```

### 命令行参数
- `-t, --target <TARGET>`: 编译目标 (默认: `all`)。
- `-o, --output <DIR>`: 指定输出目录 (默认: `target`)。
- `-v, --verbose`: 显示详细的编译日志。

## 项目结构
- `src/ast/`: AST 定义与类型转换逻辑。
- `src/lexer/`: 词法分析。
- `src/parser/`: 语法分析。
- `src/codegen/`: 代码生成器，负责将 AST 映射到 Gaia 指令或目标平台指令。
- `tests/`: 包含各种测试场景的 `.vrs` 文件。
