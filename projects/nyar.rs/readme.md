# nyar.rs

Nyar 的 Rust workspace（nightly，edition 2024）。成员按职责分五组：

| 组 | 成员 | 职责 |
|----|------|------|
| `compilers/` | `nyar`, `nyar-types`, `nyar-analyzer`, `nyar-optimizer`, `nyar-emitter`, `nyar-runner`, `nyar-workspace`, `nyar-package-manager`, `nyar-package-registry` | 语言无关的编译规划、分析、优化、产物装配与包管理 |
| `formats/` | `odin-bytecode` | 字节码磁盘合同 |
| `runtimes/` | `odin-vm`, `legend-vm`, `nyar-gc`, `nyar-jit` | VM、legacy lanes、GC、JIT |
| `languages/` | `rusty-valkyrie`（主链单包，含 `src/{legion,asgard,workspace}` 产品模块与 `src/bin/{legion,asgard}.rs`）、`rusty-{bash,c,lua,powershell,tcl}`（5 个 guest 前端） | 语言前端；其余 `rusty-*` 目录不在 workspace 的 `members` 中 |
| `ffis/` | `nyar-napi`, `nyar-wasm`, `valkyrie-napi` | 宿主绑定：通用 N-API、wasm 占位、Valkyrie 产品 N-API |

边界：Nyar 核心 crate 不得依赖 `rusty-valkyrie`、`legion`、`asgard`；`Oak` 不得依赖本 workspace。`rusty-valkyrie` 包内前端模块不得导入 `crate::{legion,asgard,workspace}`。

```powershell
cargo test -p nyar-types --features serde
cargo test -p rusty-valkyrie --lib
cargo test -p rusty-valkyrie --test package_module_boundary
```
