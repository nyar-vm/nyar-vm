# rusty-kotlin（嵌套副本）

Kotlin 前端的一份重复副本，实际位于 `languages/rusty-scala/rusty-kotlin/`，而不是 `languages/` 下的独立目录。

## 当前状态

- 本目录自带独立的 `Cargo.toml`、`src/` 与 `tests/`，但**不在 `projects/nyar.rs` 的 Cargo workspace 中**：`cargo build`、CI 与 npm 发布链都不会构建它，也没有任何 crate 依赖它。
- 与 `languages/rusty-kotlin/` 内容重复，属于误放的副本。
- 解析：`parse` 委托 `oak-kotlin` 的 builder 与 parser，产出 Oak AST。
- 降低：`lower_unified` 调用 `codegen::NyarTranslator::translate_to_id`，是一条已接线的 AST → `IKun` 转换路径；转换失败时回落为空 `Seq`。注意顶层 `languages/rusty-kotlin` 的同一调用仍被注释，两份副本行为不一致。
- 测试：`tests/` 下有 `simulator.rs` 与 `.kt` 语料，因本包不在 workspace，`cargo test` 不会执行。

## 能力边界

源码只接入 Oak 语法层与上述降低路径。旧版 readme 中出现的 `nyar-aot`、`nyar-tools`、
`nyar-jit`、`nyar-gc`、E-Graph 优化、algebraic effects、HKT / type class、标准库 FFI
等说法**在源码中不存在**。

实际能力以 `src/` 与其中的测试为准；本文件只记录当前事实。
