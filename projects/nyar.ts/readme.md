# nyar.ts

Nyar 的 pnpm workspace。三组目录：

| 路径 | 角色 |
|------|------|
| `packages/nyar` | `@nyar/nyar`：Nyar 通用宿主 API，只对接 `nyar-napi` |
| `packages/nyar-skills` | agent skills 包 |
| `platforms/` | 平台原生产物包：`@nyar/nyar-<target>`、`@nyar/valkyrie-<target>`，各含 `lib/` 下的 `.node` |
| `emulators/valkyrie` | `@nyar/valkyrie`：`legion`、`asgard` 两个 bin，直接对接 `valkyrie-napi` |

`@nyar/nyar` 不得导出或转发任何 Valkyrie / Legion / Asgard 入口；产品链只走 `@nyar/valkyrie`。

```powershell
pnpm build:napi
node projects/nyar.ts/emulators/valkyrie/bin/legion.mjs --version
```
