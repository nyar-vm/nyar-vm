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

每个前端项目通常应当遵循统一的接口规范，主要包含：

1. **Parser**: 将源代码解析为 AST。推荐使用 `oak-core` 框架。
2. **UIR Lowering**: 将 AST 转换为 `ProjectChomsky` 的 `IKun` 意图流。
3. **Compiler Driver**: 驱动 AOT/JIT 流程并输出目标构件。

## 接入新语言指南

为了保持架构的一致性并减少重复代码，接入新语言时请遵循以下步骤：

### 1. 项目结构规范

建议的前端项目结构如下：

```text
rusty-lang/
├── bin/                # 命令行工具入口 (如 langc, lang-run)
├── src/
│   ├── codegen/        # UIR 生成逻辑 (NyarTranslator)
│   ├── parser/         # 词法与语法分析 (基于 Oak)
│   ├── errors/         # 语言特定的错误处理
│   ├── lib.rs          # 导出 Frontend 实现
│   └── main.rs         # 默认编译器驱动
├── tests/              # 集成测试与示例代码
└── Cargo.toml
```

### 2. 实现 Frontend 接口 (建议)

虽然目前尚未强制，但推荐在 `lib.rs` 中定义一个 `Frontend` 结构体，并提供以下标准方法：

```rust
pub struct MyLanguageFrontend;

impl MyLanguageFrontend {
    pub fn new() -> Self;
    pub fn parse(&self, source: &str) -> MyResult<AstRoot>;
    pub fn compile_to_uir(&self, source: &str) -> MyResult<EGraph<IKun, ConstraintAnalysis>>;
    pub fn compile_to_nyar(&self, source: &str) -> MyResult<NyarModule>;
}
```

### 3. 避免过度拷贝

- **严禁跨语言拷贝代码时不修改类型名**：例如在 `Mini C#` 中使用 `JavaLanguage` 或 `JavaRoot`。
- **复用通用逻辑**：如果多个语言有相似的错误处理或 CLI 逻辑，应考虑提取到 `nyar-types` 或 `nyar-tools` 中。

### 4. 注册与集成

- 在 `documentation/zh-hans/frontends/index.md` 中添加新语言的链接。
- 在 `nyar-vm` 根目录的 `Cargo.toml` 中添加该项目到 `workspace.members`。
- (规划中) 将前端注册到 `nyar-tools` 的多语言分发器中。
