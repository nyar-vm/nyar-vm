# 常见问题

## 所有语言是否共享 Canonical MIR？

否。现有 canonical 类型承载 Valkyrie 语义，应改名并归还语言。Nyar 提供工具和中性合同，不能以共享子集继续保留语言语义总线。

## Python 与 Lua 如何编译？

以语言解释器为语义来源，执行 Futamura 第一投影，保留运行期未知操作及依赖。LegendVM 提供执行与特化基底，目标适配器分别产生目标表示。当前有限 residual 接线不构成完整支持。

## 所有目标是否经过 OdinVM？

否。OdinVM 只执行 OVM 的 `.odin`。Wasm、CLR、JVM、native 各自独立。

## N-API 包是否证明 guest 可以编译为 `.node`？

否。宿主编译器绑定与 guest 扩展是不同 ABI 链路。`.node` 和 `.pyd` 也不是共享库改后缀。

## 文档目标是否都已完成？

否。以[架构与当前状态](architecture.md)、活动 workspace、当前源码与真实产物证据共同判断。测试通过不能证明分层正确。
