# nyar-runtime

The standard runtime implementation for the Nyar virtual machine ecosystem.

## Overview

`nyar-runtime` provides a concrete implementation of the runtime traits required by `nyar-vm`. While `nyar-vm` remains runtime-agnostic to ensure maximum flexibility, `nyar-runtime` serves as a feature-rich environment suitable for most general-purpose applications.

## Features

- **Standard Library Implementation**:
  - **Filesystem**: Asynchronous file I/O operations.
  - **Networking**: TCP/UDP and HTTP client support.
  - **Serialization**: Native support for JSON, TOML, and VON (Nyar Object Notation).
- **Asynchronous Execution**: Deeply integrated with the VM's algebraic effects system for seamless async/await patterns.
- **Platform Abstraction**: Consistent API across different operating systems.
- **FFI Registry**: A comprehensive set of foreign function interfaces for common system tasks.

## Customization & Scenarios

Thanks to the decoupled architecture of the Nyar ecosystem, this runtime can be easily swapped or extended for specialized scenarios:

- **Game Scripts**: Can be stripped down to a minimal set of APIs for embedding into game engines like Cocos Creator or Unity.
- **Cloud/Internet**: Enhanced with specific web service capabilities and high-performance networking.
- **Edge Computing**: Tailored for lightweight execution on resource-constrained devices.

## Usage

To use the standard runtime with `nyar-vm`:

```rust
use nyar_vm::NyarVM;
use nyar_runtime::StandardRuntime;

fn main() {
    let runtime = StandardRuntime::new();
    let vm = NyarVM::with_runtime(runtime);
    // Execute your Gaia IR modules
}
```

## License

Licensed under MIT OR Apache-2.0.
