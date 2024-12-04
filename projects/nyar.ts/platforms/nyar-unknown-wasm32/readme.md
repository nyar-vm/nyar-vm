# @nyar/nyar-unknown-wasm32

Prebuilt **Wasm GC host sidecar** for Nyar VM. **Not a public import target.**

This package is pulled automatically when installing `@nyar/nyar` on Wasm-capable hosts.

## Example

Application code imports the main package only. The packaged `nyar.wasm` currently exports only linear memory; its host
export surface and import ABI are not frozen yet, so this package cannot execute bytecode today.

Build and publish pipelines are documented for maintainers in the repository; they are not repeated here.
