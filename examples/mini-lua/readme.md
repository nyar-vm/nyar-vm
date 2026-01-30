# Mini Lua 语言编译器

这是一个类 Lua 语言的编译器前端实现，旨在验证 Gaia 项目对动态语言语法的支持，以及生成符合 Gaia 指令集的能力。

## 核心目标

- **Lua 验证**：验证从 Lua 源代码到 Gaia 指令集的完整流程。
- **动态特性支持**：测试 Gaia 后端对 Lua 风格的动态特性（如表、闭包等）的处理。

## 支持特性

### 语言特性
- **基本类型**：数值、字符串、布尔、nil。
- **表 (Table)**：Lua 核心数据结构支持。
- **函数**：支持函数定义、匿名函数、闭包。
- **控制流**：完整的 `if-then-else`、`for`, `while`, `repeat-until` 循环。
- **局部变量**：`local` 声明支持。

### 编译器功能
- 支持生成 Gaia 指令集 (Gaia IR)。

## 快速开始

### 使用方法
```bash
# 查看抽象语法树
cargo run -- <input.lua> --ast

# 生成 Gaia 指令
cargo run -- <input.lua> --gaia
```

### 常用选项
- `--ast`: 输出抽象语法树。
- `--tokens`: 输出词法标记流。
- `--gaia`: 输出 Gaia 指令。
- `--gaia-json`: 以 JSON 格式输出 Gaia 指令。

## 项目结构
- `src/lib.rs`: Mini Lua 前端核心实现。
- `src/codegen.rs`: Gaia 指令生成器。
- `bin/luac.rs`: 命令行工具实现。
