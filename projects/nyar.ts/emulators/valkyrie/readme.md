# @nyar/valkyrie

Valkyrie npm 包装层。安装后提供 `asgard` 与 `legion` 两个命令；命令解析与编排由本包的
TypeScript CLI 层承担，编译执行经 `valkyrie-napi` 结构化 N-API 绑定调用，Rust 实现位于
`projects/nyar.rs/languages/rusty-valkyrie`，绑定 crate 为
`projects/nyar.rs/ffis/valkyrie-napi`。本包不重复实现编译逻辑。

## 依赖顺序

```text
@nyar/valkyrie ──► @nyar/valkyrie-<platform> ──► valkyrie-napi ──► rusty-valkyrie product
```

平台包按当前 OS/arch 选择；`@nyar/valkyrie` 不依赖 `@nyar/nyar`，Nyar 宿主 API
与 Valkyrie 产品面各自独立。

## 用法

```bash
legion --help
asgard --help
asgard build --static <entry.w|entry.awsl>
```

`--help` / `--version` 等 meta 命令由 TypeScript CLI 层（`@vmz/commander`）解析，不经 native。

## 命令边界

- `asgard build --static <entry>`：当前唯一可用的静态交付入口，TS 解析参数后调用结构化
  `asgardBuildStatic`。
- `asgard build`（不带 `--static`）：明确诊断并返回非 0。构建内容应由项目根
  `asgard.config.v` 的有类型求值决定，该链路（C0/C2）尚未闭合。
- `asgard serve` / `dev` / `pack` / `fmt`：尚未接入配置求值与产物管线，明确诊断并返回非 0。
- 未知子命令：TS 层直接报错并返回非 0，不透传给 native。

`process.argv` 只出现在 TypeScript CLI 层；`valkyrie-napi` 不接受 `argv`。

## 导出

- `PACKAGE_NAME` / `PACKAGE_VERSION`：来自本包 `package.json` 的 `name` / `version`
- `COMMANDS`：本包提供的命令名
- `runLegionCli` / `runAsgardCli`：命令编排入口
- `legionPlan` / `legionBuild` / `legionTest`：结构化 N-API 封装

## License

MIT
