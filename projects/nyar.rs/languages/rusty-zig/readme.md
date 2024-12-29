# rusty-zig

Zig 前端（Nyar guest 语言包）。

## 当前状态

- **不在 `projects/nyar.rs` 的 Cargo workspace 中**：`Cargo.toml` 存在，但 workspace 的 `members` 不含本包；`cargo build`、CI 与 npm 发布链都不会构建它，也没有任何 crate 依赖它。
- 包名 `rusty-zig`，`src/` 约 37 行。
- 解析：`parse` 委托 `oak-zig` 的 builder 与 parser，产出 Oak AST。
- 降低：`lower_unified` 返回占位 `0`，`codegen` 模块只有 TODO 注释。
- 测试：无 `tests/` 目录。

## 能力边界

源码只接入 Oak 语法层与上述降低路径。旧版 readme 中出现的 `nyar-aot`、`nyar-tools`、
`nyar-jit`、`nyar-gc`、E-Graph 优化、algebraic effects、HKT / type class、标准库 FFI
等说法**在源码中不存在**。

实际能力以 `src/` 与其中的测试为准；本文件只记录当前事实。