# 语言服务

本模块将编译器的源码处理能力用于开发工具，包含高亮、lint 和格式化 provider 注册。

`highlight` 提供高亮处理，`lint` 提供源码 lint 入口，`format_provider` 将 Valkyrie 打印和格式化能力接入 `nyar-format`。格式化所需的语法结构由 Oak 提供。

LSP 的协议转换、文档状态和编辑器请求处理位于 [legion/lsp](../../legion/lsp/readme.md)。从工具入口追踪到本模块，可以了解编辑器和命令行如何使用编译器的源码分析能力。
