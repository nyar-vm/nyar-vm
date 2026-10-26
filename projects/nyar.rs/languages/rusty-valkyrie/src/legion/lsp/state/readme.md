# lsp state

LSP-side caches, document state, and query helper structures.

## Responsibilities

- Track open documents, version stamps, and incremental invalidation.
- Cache compiler snapshots reused by multiple handlers.

State 保存文档查询所需的快照和缓存，语义结果仍来自 Compiler 的主链。
