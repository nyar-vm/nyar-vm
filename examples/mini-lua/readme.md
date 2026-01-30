# Mini Python 语言编译器

这是一个类 Python 语言的编译器前端实现，旨在验证 Gaia 项目对动态语言语法的支持，以及生成符合 Python 标准的 `.pyc` 字节码文件的能力。

## 核心目标

- **PYC 验证**：验证从 Python 源代码生成 `.pyc` (Python Compiled) 字节码文件的完整流程，确保与标准 Python 运行时的兼容性。
- **动态特性支持**：测试 Gaia 后端对 Python 风格的动态特性（如类、动态类型、推导式等）的处理。

## 支持特性

### 语言特性
- **面向对象**：支持 `class` 定义与继承。
- **函数式编程**：支持 `def` 定义、`lambda` 表达式。
- **控制流**：完整的 `if-elif-else`、`for-in`、`while` 循环，支持 `break`, `continue`, `pass`。
- **异常处理**：支持 `try-except-finally` 结构和 `raise`。
- **高级语法**：支持列表/字典/集合推导式 (Comprehensions)。
- **模块系统**：支持 `import` 和 `from ... import`。
- **上下文管理**：支持 `with` 语句。

### 编译器功能
- 支持生成 Gaia 指令集 (Gaia IR)。
- **重点功能**：支持生成标准 Python 字节码 (`.pyc`)。

## 快速开始

### 使用方法
```bash
# 生成 Python 字节码
cargo run -- <input.py> --pyc
```

### 常用选项
- `--pyc`: 编译为 Python 字节码文件。
- `--ast`: 输出抽象语法树。
- `--tokens`: 输出词法标记流。
- `--gaia`: 输出 Gaia 指令。
- `--gaia-json`: 以 JSON 格式输出 Gaia 指令。

## 项目结构
- `src/ast.rs`: 涵盖 Python 丰富语法的 AST 定义。
- `src/parser.rs`: 语法解析逻辑。
- `src/pyc_codegen.rs`: 专门用于生成 `.pyc` 文件的后端实现。
- `examples/`: 包含类、控制流等示例 Python 代码。
