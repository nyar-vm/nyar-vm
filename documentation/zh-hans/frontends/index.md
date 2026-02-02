# 语言前端示例

Nyar VM 提供了多种主流编程语言的简化版本（Mini-languages）作为前端示例，用于展示如何利用 `ProjectChomsky` 编译器框架将不同范式的语言映射到 Nyar IR。

## 示例列表

- [Mini C](mini-c.md): 经典的过程序编程语言。
- [Mini C#](mini-csharp.md): 现代的面向对象语言。
- [Mini Java](mini-java.md): 企业级面向对象语言。
- [Rusty Kotlin](rusty-kotlin.md): 简洁的现代多范式语言。
- [Mini Lua](mini-lua.md): 嵌入式动态脚本语言。
- [Mini Python](mini-python.md): 广泛使用的动态语言。
- [Mini TypeScript](mini-typescript.md): 带类型的 JavaScript 超集。

## 职责说明

每个前端项目应当遵循统一的接口规范，主要包含：

1. **Parser**: 将源代码解析为 typed AST。必须使用 `oak-core` 框架。
2. **Lowering**: 将 AST 转换为通用中间表示 `IKunTree`。
3. **NyarDriver Integration**: 所有的前端应当接入 `nyar-vm` 提供的 `NyarDriver`，以实现统一的 JIT 运行与 AOT 编译体验。

## 接入新语言指南

为了保持架构的一致性并减少重复代码，接入新语言时请遵循以下步骤：

### 1. 项目结构规范

建议的前端项目结构如下：

```text
rusty-lang/
├── bin/                # 命令行工具入口 (如 langc, lang-run)
├── src/
│   ├── codegen/        # 转换为 IKunTree 的逻辑
│   ├── lib.rs          # 实现 NyarFrontend trait
│   └── main.rs         # 使用 NyarDriver 的默认运行工具
├── tests/              # 所有的测试必须放在根目录的 tests/ 下
└── Cargo.toml
```

### 2. 实现 NyarFrontend Trait

在 `lib.rs` 中定义前端结构体并实现 `NyarFrontend` trait：

```rust
use nyar_error::NyarError;
use nyar_frontend::NyarFrontend;
use nyar_types::IKunTree;
use oak_mylang::{MyLanguage, MyRoot};

pub struct MyLanguageFrontend;

impl NyarFrontend for MyLanguageFrontend {
    type Language = MyLanguage;

    fn parse(&self, source: &str) -> Result<MyRoot, NyarError> {
        // 使用 oak-core 构建 AST
    }

    fn lower(&self, ast: &MyRoot) -> Result<IKunTree, NyarError> {
        // 将 AST 转换为 IKunTree
    }
}
```

### 3. 使用 NyarDriver

在 `main.rs` 或 `bin/` 工具中，使用 `NyarDriver` 来驱动执行：

```rust
fn main() {
    let frontend = MyLanguageFrontend::new();
    let driver = NyarDriver::new();
    let input_file = Path::new("test.mylang");
    
    // 直接运行源代码
    driver.run_source(&frontend, input_file).unwrap();
}
```

### 4. 避免过度拷贝

- **严禁跨语言拷贝代码时不修改类型名**：确保 AST 根节点类型与语言对应（如 `JavaRoot`, `RustRoot`）。
- **统一错误处理**：所有的错误必须统一为 `NyarError`。
- **减少 dyn 使用**：`NyarDriver` 使用静态泛型，避免在热点路径使用 trait objects。
