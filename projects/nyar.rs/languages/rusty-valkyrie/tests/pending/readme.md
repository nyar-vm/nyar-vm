# Emitter 测试样本

本目录保存使用 `FragmentSubmission` 和 `executable_provider` 接口的后端测试样本。这些文件位于正式测试模块树之外，用于查阅编译载荷与装配行为的历史用法。

`tests/frontend` 与 `tests/backend` 仍留在树中但已从默认 `[[test]]` 目标摘除（见 `languages/rusty-valkyrie/Cargo.toml`），待迁移到 `TargetExecutionPlan` / 新 MIR 信封后再重新注册。

执行范围和接口迁移安排记录在工作区的内部决策文档中。
