# lsp handlers

LSP request handlers.

## Responsibilities

- Map protocol requests to compiler queries and document state.
- Keep each handler focused on a single capability.

Handlers 负责协议参数、文档位置和编译器查询之间的转换；语义算法由 Compiler 提供，协议路由由上层入口组织。
