# @nyar/valkyrie

Valkyrie npm 包装层。安装后提供 `asgard` 与 `legion` 两个命令；命令的参数解析、
编译与执行由 `@nyar/nyar` 的平台绑定承担，Rust 实现在
`examples/rusty-valkyrie/crates`（`asgard`、`legion`、`legion-workspace`、
`vcc-wasm`）。本包不重复实现 CLI 或编译逻辑。

## 依赖顺序

```text
@nyar/valkyrie ──► @nyar/nyar ──► @nyar/nyar-<platform> ──► nyar-napi
```

`@nyar/nyar` 按当前 OS/arch 选择已安装的平台 collect，因此本包不直接依赖任何
平台包。

## 用法

```bash
legion --help
asgard --help
```

`--help` / `--version` 等选项由 Rust 侧解析，JS 入口只转发命令名与 argv。

## 导出

- `PACKAGE_NAME` / `PACKAGE_VERSION`：包装层身份与版本
- `COMMANDS`：本包提供的命令名
- `runValkyrieCli`：来自 `@nyar/nyar` 的命令执行入口

## License

MIT
