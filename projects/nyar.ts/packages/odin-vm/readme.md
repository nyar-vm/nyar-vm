# @nyar/odin-vm

OdinVM invocation surface and host CLI. **Bytecode execution** goes through `odin-vm` → `interpreter-vm-ovm`; **host capabilities** are exported from `nyar-napi` via `src/native.ts`.

## CLI

```bash
odin-vm version
odin-vm host-info
odin-vm ovm exports --module ./app.odin
odin-vm ovm invoke --module ./app.odin --entry main --args '[1, 2]'
odin-vm runtime list
odin-vm runtime prepare --family wasi --artifact ./app.wasm --entry main
```

`.odin` invocations require the ABI sidecar next to the artifact. When the sidecar is missing, unparsable, or does not
match the artifact hash, the native binding refuses to materialize structured arguments or results.

## Programmatic surface

```typescript
import {
    getHostInfo,
    invokeOdinVmExport,
    listOdinVmExports,
    listRuntimeFamilies,
    prepareRuntimeCommand,
    runHostScript,
} from '@nyar/odin-vm';

const info = getHostInfo();
const symbols = listOdinVmExports('/path/to/app.odin');
const value = invokeOdinVmExport('/path/to/app.odin', symbols[0], [1, 2]);

const cmd = prepareRuntimeCommand({family: 'wasi', artifact: '/path/to/app.wasm', entry: 'main'});
const result = runHostScript({language: 'lua', source: 'print("hi")'});
```

`runHostScript` executes host-script source through the `legend-vm` substrate in `nyar-napi`; the simulation packages
(`@nyar/bash`, `@nyar/lua`, `@nyar/tcl`) use it for their `bin` entries.

Platform collects (`odin-vm-win32-x64`, `odin-vm-linux-x64`, `odin-vm-darwin-*`, `odin-vm-unknown-wasm32`) are all
optional dependencies: the host picks the collect matching the current OS/arch, falls back to the Wasm collect when
available, and reports a clear error when none is installed.

## License

MIT
