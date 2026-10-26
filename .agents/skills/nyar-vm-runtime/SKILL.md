---
name: nyar-vm-runtime
description: >-
  改 nyar-vm 的运行时层：OdinVM（odin-vm）、LegendVM（legend-vm）、nyar-gc、
  nyar-jit、odin-bytecode。用户要求修解释器、GC、JIT、协程/代数效应、字节码格式、
  guest 语言接入或运行时架构守卫时加载。
---

# nyar-vm 运行时

## 范围

| crate | 职责 |
|-------|------|
| `runtimes/odin-vm` | OVM 字节码加载、校验、解释执行、宿主导入、JIT 注入 |
| `formats/odin-bytecode` | `.nyar` 磁盘格式的编解码合同 |
| `runtimes/legend-vm` | 语言无关的 PE / Futamura 基底、guest 注册与分发 |
| `runtimes/nyar-gc` | 托管堆、`Value`、分代标记-清除回收、写屏障 |
| `runtimes/nyar-jit` | `JitCompiler`、栈图、标量快路径、去优化 |

先读对应 crate 的 `readme.md` 与 `tests/`，再动代码。

## OdinVM 不变量

- `odin-vm` 只消费 `odin-bytecode`，不解析源码，不导入 `rusty-<lang>` 的 AST、parser 或编译流水线。
- 模块加载必须校验结构、导出与常量池；损坏输入返回错误，不进入解释循环。
- 新增或修改字节码指令时，同时改 `odin-bytecode` 的编码/解码、`odin-vm` 的 `ops`/`verify`、以及两端测试。
- 代数效应、协程与异步走 `PerformEffect` / `Yield` / `Resume` 与 `CoroutineState`；恢复点使用内码 `ExecOp` 下标。
- JIT 只通过 `Executor::with_jit` 注入，假设失效必须能经栈图去优化回解释器。
- `tests/architecture_guards.rs` 是边界守卫：为了通过测试而放宽导入限制属于反架构操作，应改实现而不是改守卫。

## LegendVM 不变量

- guest 语言（Bash / Lua / PowerShell / Tcl / C）各自在 `languages/rusty-<lang>` 中拥有 AST、语义桥和解释器；`legend-vm` 只做注册、分发和薄适配。
- 禁止在 `legend-vm` 内新增完整的语言 tree-walk，禁止引入共享的 `HostScript*` 抽象。
- 产物路径是 `build --target native` → Futamura 第一投影 → `NativeResidualModule` → PE；`.nyar` / OdinVM 只用于探针，不是宿主脚本的产品路径。
- `tests/architecture_guards.rs`、`tests/golden_futamura.rs`、`tests/lang_<lang>.rs` 必须随改动保持通过。

## GC / JIT 合同

- `nyar_gc::Value` 是带标签枚举；对象形状通过 `LayoutDescriptor` 注册，堆内按 `layout_id` 存槽位。
- 堆与回收器不得依赖语言 crate；宿主根与写屏障属于 `nyar-gc`。
- `nyar-jit` 只消费字节码级事实（函数规格、栈图、标量程序），不得依赖 `odin-vm` 或语言 crate。
- 栈图、去优化值编解码是 `odin-vm` 与 `nyar-jit` 的共享合同，改一侧必须同步另一侧与测试。

## 工作流

1. 定位 crate，读 `readme.md` 与相关 `tests/*.rs`。
2. 用最小测试复现问题；没有覆盖就先在 `tests/` 补正反用例。
3. 改实现，保持公开 API 的类型化入口，不用占位实现绕过。
4. 跑目标 crate 的测试：

```powershell
cargo test -p odin-vm
cargo test -p legend-vm
cargo test -p nyar-gc
cargo test -p nyar-jit
cargo test -p odin-bytecode
cargo test -p odin-vm --test architecture_guards
```

5. 跑全工作区回归：

```powershell
cargo test --workspace --lib
cargo test --workspace --tests --no-fail-fast
cargo fmt --all --check
```

## 常见错误

- 把语言语义塞进 `odin-vm` 或 `legend-vm`。
- 为了让新指令通过，放宽 `verify` 或架构守卫。
- 用 `#[cfg(test)]` 在 `src/` 内堆测试，或给 `Cargo.toml` 加 `[[test]]`。
- 修改 GC 的 `Value` 后没有同步 JIT 的去优化编解码。
- 把 LegendVM 的 `.nyar` 探针路径当成产品特化出口。
- 在 `legend-vm` 里加完整的语言解释器副本，而不是在 `rusty-<lang>` 里实现并注册薄适配器。
