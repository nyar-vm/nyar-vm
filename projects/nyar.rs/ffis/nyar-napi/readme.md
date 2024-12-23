# nyar-napi

Node-API (**N-API**) native binding layer exposing generic Nyar host capabilities.

## Overview

`nyar-napi` produces the native `.node` sidecar loaded by the main `nyar` npm package on matching platforms.
It re-exports `nyar-runner`. There is **no** CLI binary here.

## Build

```bash
cargo build -p nyar-napi --release
pnpm build:napi   # from repo root
```

## API surface

- Host protocol: `getHostInfo` / `HOST_PROTOCOL`.
- Runtime families: `listRuntimeFamilies` / `parseRuntimeFamily` / `prepareRuntimeCommand` / `runPreparedCommand`.
- Host scripts: `runHostScript`.

Business logic stays in Rust host modules shared with other targets.

## Boundaries

- Platform sidecar—not imported directly from app code; use the umbrella `nyar` package.
- Compilation pipelines remain in the frontend crates (`examples/rusty-<lang>`) / `nyar-emitter`.
