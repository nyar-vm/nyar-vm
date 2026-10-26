# AGENTS

本文件是面向 AI Agents 的 **nyar-vm.rs** 项目说明，如果你是人类，你应该阅读 [readme.md](./readme.md)
，如果你是开发者，你应该阅读 [documentation/readme](./documentation/readme.md)。

作为代理，在开始工作前应按任务加载 `.agents/skills/` 下的专题 skill。

## 仓库定位

本仓同时承载三块内容：

- **Nyar 编译平台**（`projects/nyar.rs/compilers`）：`nyar`、`nyar-analyzer`、`nyar-optimizer`、`nyar-emitter`、`nyar-types`、
  `nyar-runner`、包管理与 workspace。
- **nyar-vm 运行时**（`projects/nyar.rs/runtimes`）：`odin-vm`（OdinVM）、`legend-vm`（LegendVM）、`nyar-gc`、`nyar-jit`。
- **宿主与平台产物**（`projects/nyar.rs/ffis`、`projects/nyar.ts`）：`nyar-napi`、`nyar-wasm`、`valkyrie-napi` 与 npm 平台包。

维护者文档在 [`documentation/`](./documentation)。该目录当前仍有历史遗留内容（`nyar-aot`、旧目录、旧流程），与源码冲突时一律以源码、测试和
`Cargo.toml` 为准。 **不得删除该目录**，过时内容就地修正。

## 任务路由

| 任务                                      | 先读                                                                                          |
|-------------------------------------------|-----------------------------------------------------------------------------------------------|
| 改 OdinVM / LegendVM / GC / JIT / 字节码  | [`nyar-vm-runtime`](./.agents/skills/nyar-vm-runtime/SKILL.md)，以及对应 crate 的 `readme.md` |
| 改前端、MIR、优化、后端降低、产物装配     | [`nyar-vm-compiler`](./.agents/skills/nyar-vm-compiler/SKILL.md)                              |
| bump 版本、打 tag、npm 可信发布、发布排障 | [`nyar-vm-release`](./.agents/skills/nyar-vm-release/SKILL.md)                                |
| 写用户 readme / 维护者文档                | 本文「文档分工」一节                                                                          |

## 目录与归属

```text
projects/nyar.rs/
├── compilers/     语言无关的规划、分析、优化、降低与产物装配
├── formats/       odin-bytecode：.nyar 磁盘格式合同
├── runtimes/      odin-vm、legend-vm、nyar-gc、nyar-jit
├── languages/     rusty-valkyrie（主链单包）与 5 个 guest 前端
└── ffis/          nyar-napi、nyar-wasm、valkyrie-napi

projects/nyar.ts/
├── packages/      @nyar/nyar 与 skills 包
├── platforms/     按平台打包的 .node 产物
└── emulators/     @nyar/valkyrie（legion/asgard）与脚本语言模拟器
```

## 依赖边界

```text
Oak       ──不能依赖──> Nyar
Nyar      ──不能依赖──> rusty-valkyrie / Legion / Asgard
Valkyrie 前端 ──不能依赖──> Legion / Asgard
```

- `rusty-valkyrie` 是 **单一 Cargo package**：`src/` 是前端本体，`src/legion`、`src/asgard`、`src/workspace`
  是包内产品模块；包内不得出现第二个 `Cargo.toml`，前端模块不得导入产品模块。
- Nyar 核心 crate 不得依赖 `rusty-valkyrie`、`legion`、`asgard`。
- `odin-vm` 不得导入任何语言的 AST、解析器或编译流水线。
- `legend-vm` 不得新增完整的语言 tree-walk；guest 的解析与解释留在 `rusty-<lang>` crate。
- `nyar-types` 是跨前端/后端的中性合同与身份类型，不是共享的 god IR。
- **[valkyrie.rs](https://github.com/valkyrie-language/valkyrie.rs) 是下游语料仓**（fixtures / golden / harness 黑盒），经 git 依赖消费 `nyar-vm` 的 CLI 与编译栈。**本仓不得引用、写入或假设 valkyrie.rs 路径**；语义与集成 Rust 测试只留在 `rusty-valkyrie/tests/`。

## 硬性约束

1. **测试位置**：集成测试放在 crate 的 `tests/` 目录；禁止把测试迁进 `src/` 的 `#[cfg(test)]` 模块。
2. **测试发现**：禁止在 `Cargo.toml` 声明 `[[test]]`，必须使用标准的 `tests/*.rs` 自动发现。
3. **文档检查**：`missing_docs` 必须保持 `deny`，禁止用 `#[allow(missing_docs)]` 绕过；新增公开项补齐文档。
4. **工具链**：workspace 使用 nightly 与 edition 2024（crate 使用 `#![feature]`），不要做 stable 化。
5. **全量验证**：改动后至少跑全工作区 `--lib` 与 `--tests`，并保证格式化通过、无编译警告、工作树可编译。
6. **修根因**：禁止在 core 层用类型转换、占位返回、临时 stub 掩盖缺陷；每个 core 改动要有规范依据、完整需求链和正反测试。
7. **禁止魔法**：不得引入规范之外的魔法函数、硬编码特判、名字表映射；编译器严格遵循语言规范。
8. **动态值**：测试中禁止硬编码会随构建变化的哈希、指纹、校验和，必须运行时读取或自动生成。
9. **不积压**：每完成一个可验证的部分就提交，不把大量未完成改动堆在工作区。
10. **交付依赖**：本地 `.cargo/config.toml` 的 path patch 只用于本机验证；交付必须回到 `Cargo.toml` 里 pin 的 git rev
    复验，不得用本地 patch 掩盖正式链路状态。

## 构建与验证

工作目录按 CI 约定在 `projects/nyar.rs`，或使用 `--manifest-path projects/nyar.rs/Cargo.toml`。

```powershell
cargo build --workspace
cargo test  --workspace --lib
cargo test  --workspace --tests --no-fail-fast
cargo fmt --all --check
```

TypeScript / npm 侧：

```powershell
pnpm install
pnpm --filter "./projects/nyar.ts/**" test
node scripts/prepare-npm-publish.mjs
node scripts/check-nyar-workspace-versions.mjs
node scripts/build.mjs napi      # 或 wasm
```

CI 等价门槛见 `.github/workflows/check-rust.yml` 与 `check-typescript.yml`：三平台 build/test、独立 integration job。
**不得**给集成测试加 `continue-on-error`。

本机需要 Oak / Acorn 兄弟仓时，复制 `projects/nyar.rs/.cargo/config.example.toml` 为 `.cargo/config.toml`（已
gitignore），不要提交。

## 文档分工

- `readme.md` 面向用户，回答「nyar-vm 是什么」：先 OdinVM，再 OdinVM 核心特性，然后 LegendVM。保持 mermaid
  架构图，静态与动态执行路径用同构的横向结构。AOT 是编译期概念（由 `nyar-emitter` 产出），不是运行时库。
- **禁止**把给 agent 的阅读顺序、目录清单、构建/测试命令、维护者约束写进 `readme.md`。
- 面向代理的常驻规则写本文件，专题流程写 `.agents/skills/<name>/SKILL.md`，维护者文档写 `documentation/`。
- 不新增 `docs/`、roadmap、handoff 或历史进度文档；过期内容直接修正，不留归档副本。
- 单一事实只写一处：跨仓结构写 `projects/readme.md`，crate 细节写各自 `readme.md`。

## Git 约定

- subject 以真实 gitmoji 字符开头，subject 与 body 用英文；subject 末尾不加句号。
- 标识符用反引号包起来（路径、类型、函数、crate 名）。
- commit message 不写版本号、不写内部计划/里程碑代号。
- Windows 下带 emoji 的 commit message 用 UTF-8 文件 + `git commit -F`，不要用 PowerShell here-string 直接 `-m`。
- 发布 tag 只能打在 CI 全绿的提交上；真实版本只经 GitHub Actions Trusted Publisher（OIDC）发布，不在本地发布。

## 代理检索纪律

- 不要在仓库根做无界 glob；`node_modules/`、`projects/nyar.rs/target/`、`dist/` 会拖垮会话。
- 已知 crate → 直读该 crate 的 `src/`、`tests/` 与 `readme.md`。
- 搜符号先限定到 `compilers/`、`runtimes/`、`languages/rusty-valkyrie/` 等具体目录，并限制结果条数。
- 需要跨 crate 约定时读 `projects/readme.md` 与 `documentation/`，再回到源码核对。

## 常见陷阱

- `languages/` 下只有 `rusty-valkyrie` 与 `rusty-{bash,c,lua,powershell,tcl}` 在 workspace `members` 中；其余 `rusty-*`
  目录不参与构建。
- `documentation/` 的旧页面可能仍写 `nyar-aot`、`nyar-tools`、`ProjectChomsky` 或 `examples/rusty-*`；这些是历史内容，不代表当前代码。
- `legend-vm` 的原生产物路径是 Futamura 特化 → `NativeResidualModule` → PE；`.nyar` / OdinVM 只作探针，不是宿主脚本产品路径。
- `odin-vm` 只消费 `odin-bytecode`，不解析源码，也不拥有 `.nyar` 的磁盘格式合同。
- `projects/nyar.rs/.cargo/config.toml` 是本机文件，永远不要提交。
- npm 平台包命名固定为 `@nyar/<包名>-<平台>`，不要加 `cli`、`gnu`、`musl` 等额外后缀。

## Skills

| Skill                                                            | 用途                                              |
|------------------------------------------------------------------|---------------------------------------------------|
| [`nyar-vm-runtime`](./.agents/skills/nyar-vm-runtime/SKILL.md)   | OdinVM、LegendVM、GC、JIT、字节码的改动与架构守卫 |
| [`nyar-vm-compiler`](./.agents/skills/nyar-vm-compiler/SKILL.md) | 前端、MIR、优化、目标降低与产物装配               |
| [`nyar-vm-release`](./.agents/skills/nyar-vm-release/SKILL.md)   | 版本 bump、tag、npm 可信发布与排障                |
