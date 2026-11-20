# nyar.rs

Nyar 的 Rust workspace（nightly，edition 2024）。成员按职责分四组：

| 组 | 成员 | 职责 |
|----|------|------|
| `compilers/` | `nyar`, `nyar-types`, `nyar-analyzer`, `nyar-optimizer`, `nyar-emitter`, `nyar-runner`, `nyar-workspace`, `nyar-package-manager`, `nyar-package-registry` | 语言无关的编译规划、分析、优化、产物装配与包管理 |
| `runtimes/` | `legend-vm` | guest execution and specialization（托管堆与 JIT 底座已迁至 Boost `interpreter-gc` / `interpreter-jit`，以 git `rev` 消费） |
| `languages/` | `rusty-{bash,c,lua,powershell,tcl}`（5 个 guest 前端） | 语言前端；其余 `rusty-*` 目录不在 workspace 的 `members` 中 |
| `ffis/` | `nyar-napi`, `nyar-wasm` | 宿主绑定：通用 N-API、wasm 占位；Valkyrie 产品 N-API 已迁至 `valkyrie.rs` |

边界：Nyar 核心 crate 不得依赖 Valkyrie 产品（`legion`、`asgard` 归 `valkyrie.rs`）；`Oak` 不得依赖本 workspace。OdinVM 已独立成仓（`nyar-vm/odin-vm`），本 workspace 以 git `rev` 消费 `odin-vm`。

```powershell
cargo test -p nyar-types --features serde
cargo test -p legend-vm
```
