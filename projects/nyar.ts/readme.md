# nyar.ts

Nyar 的 pnpm workspace。三组目录：

| 路径 | 角色 |
|------|------|
| `packages/nyar` | `@nyar/nyar`：Nyar 通用宿主 API，只对接 `nyar-napi` |
| `packages/nyar-skills` | agent skills 包 |
| `platforms/` | 平台原生产物包：`@nyar/nyar-<target>`，各含 `lib/` 下的 `.node` |
| `emulators/` | bash / lua / tcl 模拟器 |

`@nyar/nyar` 不得导出或转发任何 Valkyrie / Legion / Asgard 入口；Valkyrie 产品包装层
`@nyar/valkyrie`、平台包与 `valkyrie-napi` 已迁至 `valkyrie.rs`。

```powershell
pnpm build:napi
```
