# nyar-napi

Node-API (**N-API**) native binding layer for the Nyar runtime and Valkyrie compiler CLIs.

## Overview

`nyar-napi` produces the native `.node` sidecar loaded by the main `nyar` npm package on matching platforms.
It re-exports `nyar-runner`. There is **no** CLI binary here—argv parsing and command dispatch stay in
`legion::cli` / `asgard::cli`, while npm command entry is assembled in `projects/nyar.ts/emulators/valkyrie`.

## Build

```bash
cargo build -p nyar-napi --release
pnpm build:napi   # from repo root
```

## API surface

- Host capabilities: `getHostInfo` / `listRuntimeFamilies` / `prepareRuntimeCommand` / `runPreparedCommand` / `runHostScript`.
- Valkyrie CLI: `runValkyrieCli(product, argv)` forwards argv to `legion::cli::run_with_argv` or
  `asgard::cli::run_with_argv` and returns the process exit code.
- Legion programmatic surface: `legionPlan` / `legionBuild`; legacy `legionRun` / `legionRunCaptured` /
  `legionSpyRun` / `legionParse` stay available for existing collections.

Business logic stays in Rust host modules shared with other targets.

## Boundaries

- Platform sidecar—not imported directly from app code; use the umbrella `nyar` package.
- Compilation pipelines remain in the frontend crates (`examples/rusty-<lang>`) / `nyar-emitter`.
