# Nyar

Nyar 是多语言前端 + 多后端编译平台。语言前端把已闭合的语义合同交给 Nyar 平台，平台完成与语言无关的规划、降低和产物装配；`nyar`/`odin-vm` 只是其中一个后端，不是所有语言的公共 IR。

本文件是唯一仓库入口。**阅读顺序见下，按需展开各 crate 的 `readme.md`，不要一次全读。**

## 给 agent 的阅读顺序

1. 本文件（边界 + 目录 + 构建）。
2. 需要改动某个 crate 时，只读该 crate 的 `readme.md`。
3. 需要跨 crate 契约时，读 `projects/readme.md`（仓库结构）与对应 crate 的公开类型。
4. 设计决策不在本仓库维护；实现状态以源码、测试和 `git log` 为准。

## 三条不可违反的依赖边界

```text
Oak       ──不能知道──> Nyar
Nyar      ──不能知道──> Valkyrie / Legion / Asgard
Valkyrie  ──不能知道──> Legion / Asgard
```

允许的方向：

```text
rusty-valkyrie frontend  ──> Oak，以及 Nyar 的中性 frontend/backend 合同
rusty-valkyrie::legion   ──> rusty-valkyrie frontend（同包内产品模块）
rusty-valkyrie::asgard   ──> rusty-valkyrie frontend（同包内产品模块）
```

`rusty-valkyrie` 是**单一 Cargo package**：`src/` 为前端本体，`src/legion`、`src/asgard`、`src/workspace` 为包内产品模块，`src/bin/{legion,asgard}.rs` 为两个产品 bin。包内不得再出现第二个 `Cargo.toml`，前端模块不得导入产品模块。

## 仓库结构

```text
projects/
├── nyar.rs/                     Cargo workspace（nightly，edition 2024）
│   ├── compilers/               nyar, nyar-types, nyar-analyzer, nyar-optimizer,
│   │                            nyar-emitter, nyar-runner, nyar-workspace,
│   │                            nyar-package-manager, nyar-package-registry
│   ├── formats/odin-bytecode    字节码磁盘合同
│   ├── runtimes/                odin-vm, legend-vm, nyar-gc, nyar-jit
│   ├── languages/               rusty-valkyrie（主链）与 5 个 guest 前端
│   │                            （bash/c/lua/powershell/tcl）；其余 rusty-* 不在 workspace
│   └── ffis/                    nyar-napi（通用宿主）, nyar-wasm, valkyrie-napi（产品宿主）
└── nyar.ts/                     pnpm workspace
    ├── packages/                nyar（Nyar 宿主 API）, nyar-skills
    ├── platforms/               nyar-<target> / valkyrie-<target> 原生产物包
    └── emulators/               valkyrie（legion/asgard 两个 bin）, bash, lua, tcl
```

npm 侧两条链互相独立：

```text
@nyar/nyar      ──> @nyar/nyar-<platform>      ──> nyar-napi      ──> 通用宿主/runtime
@nyar/valkyrie  ──> @nyar/valkyrie-<platform>  ──> valkyrie-napi  ──> rusty_valkyrie::{legion,asgard}
```

## 构建与测试

需要 nightly 工具链（crate 使用 `#![feature]`）。

```powershell
cargo build --manifest-path projects/nyar.rs/Cargo.toml
cargo test  -p <crate>          # 单 crate 测试
pnpm build:napi                 # 构建 nyar-napi 与 valkyrie-napi 的 .node 产物
pnpm test                       # TS 包测试
```

产品 CLI 冒烟：

```powershell
node projects/nyar.ts/emulators/valkyrie/bin/legion.mjs --version
node projects/nyar.ts/emulators/valkyrie/bin/asgard.mjs --version
```

## 发布

- 真实版本只经 GitHub Actions Trusted Publisher（OIDC）发布，触发方式是 push tag；不在本地发布真实版本。
- 发布顺序：`valkyrie-<target>` → `nyar-<target>` / wasm → 主包 → `@nyar/valkyrie`。
- 版本、tag 与 workflow 合同见 `scripts/ci/publish-npm.mjs` 与 `.github/workflows/publish-npm.yml`。

## 文档规则

- 设计真相源不在本仓库；本仓库文档只描述公开结构、构建方式与 crate 职责。
- 不新增 `docs/`、design、roadmap、handoff 或历史进度文档；过期内容直接删除或改写，不留归档副本。
- 单一事实只写一处：跨 crate 结构写 `projects/readme.md`，crate 内部细节写各自 `readme.md`。
- `languages/` 下只有 `rusty-valkyrie` 与 5 个 guest 前端（bash/c/lua/powershell/tcl）在 workspace 中；其余 `rusty-*` 目录不被构建、无人消费，其 `readme.md` 首段即声明这一事实。判断能力一律以源码与测试为准。
