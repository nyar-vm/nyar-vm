# 维护入口

跨语言架构只在[架构合同](../architecture.md)定义，具体实现接口在各 crate readme 与源码。旧的独立 AOT、共享 IR、工具集合和运行时设计稿已经合并移除，不再作为并行合同。

从仓库根使用：

```powershell
cargo build --manifest-path projects/nyar.rs/Cargo.toml --workspace
cargo test --manifest-path projects/nyar.rs/Cargo.toml --workspace --lib
cargo test --manifest-path projects/nyar.rs/Cargo.toml --workspace --tests --no-fail-fast
cargo fmt --manifest-path projects/nyar.rs/Cargo.toml --all --check
```

先检查 Git 状态并归因外部改动。文档修改验证链接和差异格式；实现修改再按变更范围运行相应测试与 CI 门，不以测试绿色覆盖架构越界。

- [Nyar 编排](../../projects/nyar.rs/compilers/nyar/readme.md)
- [Emitter](../../projects/nyar.rs/compilers/nyar-emitter/readme.md)
- [LegendVM](../../projects/nyar.rs/runtimes/legend-vm/readme.md)
- [OdinVM](../../projects/nyar.rs/runtimes/odin-vm/readme.md)
- [GC](../../projects/nyar.rs/runtimes/nyar-gc/readme.md)
- [JIT](../../projects/nyar.rs/runtimes/nyar-jit/readme.md)
