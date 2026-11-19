# @nyar/odin-vm-linux-x64

Prebuilt **native OdinVM sidecar** for Linux x64. **Not a public import target.**

This package is pulled automatically when installing `@nyar/odin-vm` on a matching host.

## Example

Application code imports the main package only:

```typescript
import { invokeOdinVmExport, listOdinVmExports } from "@nyar/odin-vm";

const symbols = listOdinVmExports("/path/to/app.odin");
const result = invokeOdinVmExport("/path/to/app.odin", symbols[0]);
```

`.odin` modules require the ABI sidecar written next to the artifact; calls without a matching sidecar are rejected.

Build and publish pipelines are documented for maintainers in the repository; they are not repeated here.
