---
name: nyar-vm-compiler
description: >-
  改 nyar-vm 的编译链：HIR、Semantic MIR、CanonicalProgram、nyar-analyzer、
  nyar-optimizer、nyar-emitter、rusty-valkyrie 前端与目标降低/产物装配。
  用户要求修语义、类型、泛型、调用闭合、后端 lowering、导出或产物时加载。
---

# nyar-vm 编译链

## 流水线

```text
源码
  → 前端（rusty-valkyrie / rusty-<lang>）：解析、名字解析、类型与语义闭合
  → HIR → Semantic MIR (SSA) → CanonicalProgram → RepresentationPlan
  → nyar-analyzer：ProgramFacts
  → nyar：对象代数程序 → E-Graph 优化 → Futamura 投影计划 → ArtifactPartitionPlan
  → 目标降低（target-specific LIR / backend input）
  → nyar-emitter：validate → compile → OutputSpec → ArtifactSet
```

前端拥有语言语义；平台只做语言无关的规划、优化与装配。

## 归属

| 位置 | 拥有 |
|------|------|
| `languages/rusty-valkyrie` | Valkyrie HIR / MIR / 规范程序与表示计划；legion/asgard 产品模块 |
| `compilers/nyar-analyzer` | 前端事实到 `ProgramFacts` |
| `compilers/nyar-optimizer` | 对象代数与 E-Graph 优化会话 |
| `compilers/nyar` | 分区、后端选择、验证与编译编排 |
| `compilers/nyar-emitter` | 各后端 family 的目标降低与产物装配（CLR / JVM / WASM / native / OVM） |
| `compilers/nyar-types` | 跨前端/后端的中性身份与合同类型 |
| `formats/odin-bytecode` | `.nyar` 磁盘格式，仅 OVM 后端 |

## 不变量

- 前端在进入平台前闭合语义；后端不得回补语言语义。
- 不构造跨 target 的 god IR；各后端有自己的 lowering 与产物合同。
- target-agnostic 层不得存放具体 host 的实现细节。
- 调用必须在 HIR 阶段闭合；禁止在 MIR 层保留 deferred member 或后置泛型物化旁路。
- 错误类型全链保留原错误身份、参数与定位，禁止 `to_string()` 降级为纯文本。
- 禁止规范外的魔法函数、名字表映射、硬编码特判；缺事实时修上游证据，不扩大匹配范围。
- `rusty-valkyrie` 前端模块不得导入 `crate::{legion,asgard,workspace}`；包内不得新增第二个 `Cargo.toml`。
- `languages/` 只有 `rusty-valkyrie` 与 `rusty-{bash,c,lua,powershell,tcl}` 在 workspace 中；其余 `rusty-*` 不参与构建。

## 工作流

1. 确认改动属于哪一层；读该 crate 的 `readme.md`、`src/` 与 `tests/`。
2. 用现有 fixture 或测试复现；没有覆盖时在 crate 的 `tests/` 补正反用例。
3. 修上游事实链，不在下游加入绕过逻辑。
4. 逐层验证：

```powershell
cargo test -p rusty-valkyrie --lib
cargo test -p rusty-valkyrie --tests
cargo test -p nyar-analyzer
cargo test -p nyar-optimizer
cargo test -p nyar
cargo test -p nyar-emitter
```

5. 全工作区回归：

```powershell
cargo test --workspace --lib
cargo test --workspace --tests --no-fail-fast
cargo fmt --all --check
```

## 常见错误

- 在 `nyar-emitter` 里补语言语义，或在前端里补目标 host 细节。
- 候选匹配放宽到跨 namespace、跨包、未声明依赖的同名符号。
- 用类型转换、占位返回或临时 stub 让编译器过测试。
- 修改公开错误类型时丢失结构化信息，只保留字符串。
- 把测试放进 `src/` 的 `#[cfg(test)]`，或添加 `[[test]]` 配置。
- 把 `documentation/` 里的旧 `nyar-aot` / `nyar-tools` 流程当成当前实现。
