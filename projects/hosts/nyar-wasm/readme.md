# nyar-wasm

WebAssembly **cdylib/rlib** binding layer for the Nyar runtime.

## Overview

`nyar-wasm` builds the Wasm GC-facing artifact consumed by `@nyar/nyar` platform packages (`nyar-unknown-wasm32`). It
re-exports `nyar-runner` for shared host dispatch. There is **no** `[[bin]]` or `main` in this crate—the user-facing CLI
lives under `projects/packages/`.

## Build

```bash
cargo build -p nyar-wasm --release
pnpm build:wasm   # from projects/packages — collects into platform npm packages
```

## API surface

Currently exposes only a `wasm_placeholder` marker: no host function is exported and the Wasm host import/export ABI is
not frozen. Actual lowering and module bytes are produced by the Nyar compile pipeline, not hand-written in this crate.

## Boundaries

- Not a public import target for application code—install the main `nyar` npm package instead.
- Does not implement the Valkyrie compiler (see `examples/rusty-valkyrie`).
