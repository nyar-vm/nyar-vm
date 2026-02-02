# Rusty Go

Rusty Go 专注于展示 Nyar VM 对高并发语义和结构化类型系统（Structural Typing）的支持。

## 核心特性

- **并发模型**: 利用 Nyar VM 的 `Continuation` 和 `Effect` 系统模拟 `goroutine` 和 `channel`。
- **接口模型**: Go 的非侵入式接口（Duck Typing 风格的接口）映射到 Nyar 的特征系统。
- **快速启动**: 优化的编译路径，适合短生命周期的工具开发。

## 编译与 Lowering 流程

1.  **解析**: 使用 `oak-go` 获取源代码结构。
2.  **接口绑定**: 在运行时动态计算接口满足性，并生成相应的见证表。
3.  **并发降级**: `go` 关键字被降级为 VM 的协程创建指令。

## 对象模型处理

- **结构体**: 映射到 `ValueTag::Object`。
- **接口**: 类似于 Rust 的特征对象，但在进入 VM 前由编译器或运行时进行隐式转换。

## 实现进度

- [x] 语法解析
- [x] 结构体定义
- [x] 基础接口分发
- [ ] 完整的 Goroutine 调度器集成
- [ ] Channel 通讯语义
