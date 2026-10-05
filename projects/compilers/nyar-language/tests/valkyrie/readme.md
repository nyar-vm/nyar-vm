# Nyar-language Valkyrie 集成测试（暂停注册）

本目录保留历史端到端用例，**不再**作为 `[[test]]` 挂入 `cargo test --workspace`。

规范位置：

- Oak / 文本前端 golden：`valkyrie.rs` → `projects/compilers/valkyrie-fixtures`
- Nyar 编译器合同：逐步迁回 `nyar-language` 内联 `#[cfg(feature = "contract-gate")]` 测试，或对应 integration probe

恢复步骤：修编译错误后重新在 `Cargo.toml` 添加 `[[test]] name = "valkyrie"`。
