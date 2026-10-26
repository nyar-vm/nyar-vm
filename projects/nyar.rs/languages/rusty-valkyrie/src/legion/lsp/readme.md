# lsp src

Language Server Protocol implementation for Valkyrie.

## Responsibilities

- Organize protocol entry, handlers, state cache, and diagnostic conversion.
- Reuse compiler facts to power IDE queries.

LSP 使用 Compiler 的语法、语义和语言服务能力；协议层的数据结构只在请求处理和文档状态之间流动。
