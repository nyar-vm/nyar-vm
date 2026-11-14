# Guest 与宿主扩展

语言 guest 在自身 crate 实现解析、语义与解释器，通过薄适配器注册到 LegendVM。新增 guest 不应复制解释器到 LegendVM，也不应修改 OdinVM 使其解析源码。

宿主导入必须声明身份、签名、调用约定、能力与生命周期。动态解析是语言或宿主合同中的正式能力，不能在 emitter 中按名称猜测目标符号。

Guest 生成的 `.node` 与 `.pyd` / Python `.so` 分别满足 Node-API 与 CPython 扩展 ABI。编译器本身的绑定包是另一项交付。详见[Native 合同](../backends/native.md)与[架构合同](../architecture.md)。
