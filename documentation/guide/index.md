# 使用与验证

先确认[架构与当前边界](../architecture.md)，再选择语言和目标。不要从实验目录、旧教程或产物后缀推断能力。

Rust 构建入口是 `projects/nyar.rs/Cargo.toml`。先读语言 crate 与目标 crate 的 readme，使用其当前 CLI 帮助与测试入口。具体运行参数以实际 CLI 为准，不维护另一套假想命令。

验证源码解析、语言语义、所用分析或特化、目标适配、产物加载与执行的完整链路。动态语言必须同时验证运行期未知输入；编译期只执行一次并折叠结果不能作为等价性证据。

- [动态语言](dynamic.md)
- [扩展边界](plugins.md)
- [维护与构建](../maintenance/index.md)
