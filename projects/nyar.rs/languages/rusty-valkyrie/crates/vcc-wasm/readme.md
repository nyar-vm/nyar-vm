# vcc-wasm

Valkyrie Compiler Collect — **Wasm GC 绑定层（纯库）**。

- 仅 `cdylib` / `rlib`， **无** `[[bin]]`、 **无** `main`。
- 当前只暴露 `wasm_placeholder` 标记：Wasm 宿主导出面与 import ABI 尚未冻结。
- `legion build` 产出的 Node target 产物与 cargo 构建的 `vcc-wasm` 是两条独立链路，本 crate 不负责装配前者。
- 用户 CLI 入口位于 `projects/nyar.rs/languages/rusty-valkyrie/crates/legion` 与 `projects/nyar.rs/languages/rusty-valkyrie/crates/asgard`。

```bash
cargo build -p vcc-wasm --release
```
