# @nyar/valkyrie

Valkyrie npm 包装层。安装后提供 `asgard` 与 `legion` 两个命令；命令的参数解析、
编译与执行由 `valkyrie-napi` 原生绑定承担，Rust 实现位于
`projects/nyar.rs/languages/rusty-valkyrie`，绑定 crate 为
`projects/nyar.rs/ffis/valkyrie-napi`。本包不重复实现 CLI 或编译逻辑。

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
```

`--help` / `--version` 等选项由 Rust 侧解析，JS 入口只转发命令名与 argv。

## 导出

- `PACKAGE_NAME` / `PACKAGE_VERSION`：来自本包 `package.json` 的 `name` / `version`
- `COMMANDS`：本包提供的命令名
- `runValkyrieCli`：来自本包 `valkyrie-napi` 绑定的命令执行入口

## License

MIT
