# nyar-types

Core type definitions and common abstractions for the Nyar VM.

## Overview

`nyar-types` provides the essential data structures, traits, and error types used across the entire Nyar ecosystem. It ensures a consistent interface for language frontends, compilers, and the runtime.

## Key Components

- **`QualifiedName`**: A robust representation of hierarchical names (e.g., `std::collections::HashMap`), used for symbol lookups and module management.
- **`SourceLocation`**: Tracks the origin of code (source ID and offset) for precise error reporting and debugging.
- **`NyarFrontend` Trait**: The primary interface for language frontends. It defines how to parse source code, lower it to Gaia IR, and integrate with the VM's optimization pipeline.
- **`NyarError`**: A comprehensive error type covering syntax errors, runtime exceptions, and VM-internal failures.
- **`EffectInfo`**: Metadata for algebraic effects, used by the effect handler system.
- **VFS (Virtual File System) Abstractions**: Common traits for accessing source code and assets across different environments (disk, memory, network).

## Usage for Frontend Developers

To implement a new language frontend, implement the `NyarFrontend` trait:

```rust
use nyar_types::{NyarFrontend, NyarError, NyarContext, Id};
use oak_core::Language;

pub struct MyLanguageFrontend;

impl NyarFrontend for MyLanguageFrontend {
    type Language = MyLanguage;

    fn parse(&self, source: &str) -> Result<TypedRoot, NyarError> {
        // Implementation
    }

    fn lower_unified<V: Vfs>(&self, ast: &TypedRoot, ctx: &mut NyarContext<V>) -> Id {
        // Implementation
    }
}
```

## License

Licensed under MIT OR Apache-2.0.
