# Projects layout

本仓库只分两个 workspace：`nyar.rs`（Rust）与 `nyar.ts`（TypeScript / npm）。两侧产物通过 `.node`、wasm 与 npm 包交接，不共享构建系统。

## nyar.rs

| 路径 | 角色 |
|------|------|
| `compilers/` | Nyar 编译栈：`nyar`、`nyar-types`、`nyar-analyzer`、`nyar-optimizer`、`nyar-emitter`、`nyar-runner`、`nyar-workspace`、包管理与注册表 |
| `formats/` | 磁盘字节码合同（`odin-bytecode`） |
| `runtimes/` | `odin-vm`（VM）、`legend-vm`（legacy lanes）、`nyar-gc`、`nyar-jit` |
| `languages/` | 各语言前端 `rusty-<lang>`；主链 `rusty-valkyrie` 是单包，含 `legion`/`asgard` 产品模块与两个 bin |
| `ffis/` | 宿主绑定：`nyar-napi`（通用）、`nyar-wasm`（占位）、`valkyrie-napi`（Valkyrie 产品） |

## Dependency boundaries

- **文本与二进制格式** — Oak 拥有文本解析与恢复；Acorn 拥有外部二进制格式模型与编解码。
- **前端** — 每个语言前端独立在 `languages/rusty-<lang>`；前端拥有语言语义事实，只向平台交付已闭合合同。前端可以使用 Oak 解析器，Oak 不反向依赖 Nyar 或任何产品。
- **平台降低** — `nyar` / `nyar-emitter` 在前端合同之后拥有语言无关的规划、目标降低与产物准备；运行时包只消费已验证产物。
- **产品层** — `legion` 拥有 manifest、包图、缓存与 ArtifactStore；`asgard` 拥有 AWSL 管线、UI 打包与部署产物。两者都只在 `rusty-valkyrie` 包内，不进入 Nyar。
- **宿主** — `nyar-napi` 只暴露通用宿主/runtime；Valkyrie 产品入口在 `valkyrie-napi`，npm 侧由 `@nyar/valkyrie` 直接对接。

## nyar.ts

| 路径 | 角色 |
|------|------|
| `packages/` | `@nyar/nyar`（Nyar 宿主 API）与 skills 包 |
| `platforms/` | 按平台打包的原生 `.node` 产物：`@nyar/nyar-<target>`、`@nyar/valkyrie-<target>` |
| `emulators/` | `@nyar/valkyrie`（`legion`/`asgard` bin）与 bash/lua/tcl 模拟器 |

## Optional integration tests

部分测试可通过环境变量指向外部 Valkyrie 语言 checkout（**发布不需要**）：

```bash
export VALKYRIE_V_ROOT=/path/to/valkyrie-language-checkout
cargo test -p rusty-valkyrie
```

未设置时相关用例跳过，默认 CI 不受影响。

Per-crate 文档在各 crate 的 `readme.md`（例如 [`odin-vm/readme.md`](./nyar.rs/runtimes/odin-vm/readme.md)）。
