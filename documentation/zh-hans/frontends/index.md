# 语言前端示例

Nyar VM 提供了多种主流编程语言的简化版本（Mini-languages）作为前端示例，用于展示如何利用 `ProjectChomsky` 编译器框架将不同范式的语言映射到 Nyar IR。

## 示例列表

- [Mini C](mini-c.md): 经典的过程序编程语言。
- [Mini C#](mini-csharp.md): 现代的面向对象语言。
- [Mini Java](mini-java.md): 企业级面向对象语言。
- [Mini Kotlin](mini-kotlin.md): 简洁的现代多范式语言。
- [Mini Lua](mini-lua.md): 嵌入式动态脚本语言。
- [Mini Python](mini-python.md): 广泛使用的动态语言。
- [Mini TypeScript](mini-typescript.md): 带类型的 JavaScript 超集。

## 职责说明

每个前端项目通常包含：
1. **Parser**: 将源代码解析为 AST。
2. **UIR Lowering**: 将 AST 转换为 `ProjectChomsky` 的 `IKun` 意图流。
3. **Compiler Driver**: 驱动 AOT/JIT 流程并输出目标构件。
