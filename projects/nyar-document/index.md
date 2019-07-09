---
layout: home

hero:
  name: "Valkyrie"
  text: "现代函数式编程语言"
  tagline: 融合代数效应与强类型系统的下一代编程语言
  image:
    src: /logo.svg
    alt: Valkyrie
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: 查看示例
      link: /examples/

features:
  - icon: 🎭
    title: 代数效应系统
    details: 原生支持代数效应，优雅处理异常、异步、状态管理等副作用，提供比传统异常处理更强大和灵活的控制流抽象。
  - icon: 🔒
    title: 强类型系统
    details: 先进的类型系统支持泛型、类型推导、模式匹配，在编译时捕获更多错误，提供卓越的开发体验和代码安全性。
  - icon: 🚀
    title: 多目标编译
    details: 编译到WebAssembly、JavaScript和原生代码，一套代码运行在浏览器、服务器和桌面环境，真正实现"一次编写，到处运行"。
  - icon: ⚡️
    title: 函数式编程
    details: 不可变数据结构、高阶函数、尾调用优化等函数式特性，结合现代语法设计，让代码更简洁、更可靠、更易维护。
  - icon: 🛠️
    title: 现代工具链
    details: 集成的包管理器、格式化工具、语言服务器，提供完整的开发生态系统，支持增量编译和智能代码补全。
  - icon: 🌐
    title: 渐进式采用
    details: 可与现有JavaScript/TypeScript项目无缝集成，支持渐进式迁移，降低学习成本和迁移风险。
---

## 什么是 Valkyrie？

Valkyrie 是一门现代的函数式编程语言，专为构建可靠、高性能的应用程序而设计。它将代数效应系统与强类型系统相结合，为开发者提供了一种全新的编程体验。

### 核心特性

- **代数效应**: 优雅处理副作用，统一异常、异步、状态管理
- **强类型系统**: 编译时错误检查，支持类型推导和模式匹配
- **多目标编译**: 编译到WebAssembly、JavaScript和原生代码
- **函数式编程**: 不可变数据、高阶函数、尾调用优化
- **现代语法**: 简洁表达力强的语法设计
- **渐进式采用**: 与现有生态系统无缝集成

### 快速示例

```valkyrie
// 定义代数效应
effect Http {
    get(url: String): String
    post(url: String, body: String): String
}

// 使用效应的函数
fn fetch_user_data(id: Int) -> User {
    let response = perform Http.get(`/api/users/${id}`);
    parse_json(response)
}

// 效应处理器
fn main() {
    handle fetch_user_data(42) with Http {
        get(url) -> resume(http_client.get(url)),
        post(url, body) -> resume(http_client.post(url, body))
    }
}

// 模式匹配和类型安全
match user {
    Some(u) if u.age >= 18 -> println("成年用户: ${u.name}"),
    Some(u) -> println("未成年用户: ${u.name}"),
    None -> println("用户不存在")
}
```

这个简单的 Valkyrie 程序展示了：
- 代数效应的定义和使用
- 强类型系统和类型推导
- 模式匹配和条件守卫
- 字符串插值和现代语法

## 开始使用

准备好体验 Valkyrie 的强大功能了吗？

[快速开始 →](/guide/getting-started)

## 为什么选择 Valkyrie？

### 🎯 **解决真实问题**
传统编程语言在处理副作用时往往力不从心。Valkyrie 的代数效应系统提供了一种优雅的解决方案，让异常处理、异步编程、状态管理变得简单而强大。

### 🔧 **现代化设计**
Valkyrie 从零开始设计，吸收了函数式编程、类型理论和编程语言设计的最新成果，提供了一种既强大又易用的编程体验。

### 🌍 **广泛适用**
无论是前端应用、后端服务还是系统工具，Valkyrie 都能胜任。多目标编译能力让你的代码可以运行在任何平台上。

### 🚀 **性能优异**
先进的编译器优化技术，包括尾调用优化、内联、死代码消除等，确保生成的代码具有出色的运行时性能。