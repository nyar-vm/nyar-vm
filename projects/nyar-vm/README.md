# nyar-vm

The core execution engine for the Nyar virtual machine ecosystem.

## Overview

`nyar-vm` is a high-performance, stack-based virtual machine designed to execute Gaia Intermediate Representation (Gaia IR). It serves as the foundation for a wide range of language frontends, providing a robust and efficient runtime for both functional and object-oriented programming paradigms.

## Key Features

- **NaN-Boxing Value Representation**: Efficiently stores integers, booleans, pointers, and small strings within a 64-bit double-precision floating-point format.
- **Algebraic Effects**: Built-in support for delimited continuations and effect handlers, enabling powerful control flow abstractions like async/await, generators, and backtracking.
- **Advanced Bytecode (Gaia IR)**: A comprehensive instruction set covering:
  - Arithmetic and bitwise operations for various types.
  - Object-oriented features (fields, methods, virtual calls).
  - Functional constructs (first-class closures, upvalues, tail calls).
  - Metaprogramming (quotes, splices, macros).
  - Logic programming primitives.
- **Multi-Tier Execution**: Seamlessly transitions from an efficient bytecode interpreter to JIT-compiled native code.
- **Concurrency & Async**: Native support for futures and non-blocking execution.
- **FFI & Platform Integration**: Extensible registry for foreign function interfaces and platform-specific capabilities.

## Architecture

The VM is built with several key components:
- **Environment (`NyarEnv`)**: Manages loaded modules, symbol tables, and global builtins.
- **Core VM (`NyarVM`)**: Maintains the execution stack, call frames, and handler stacks for algebraic effects.
- **Value System**: Implements a uniform 64-bit value type with integrated garbage collection tracing.
- **JIT Provider**: An interface for plugging in various JIT compilers (like `nyar-jit`).

## Getting Started

### Prerequisites

- Rust (latest stable or nightly)
- Cargo

### Usage

To integrate the VM into your project:

```rust
use nyar_vm::vm::core::NyarVM;
use std::sync::Arc;

fn main() {
    let vm = NyarVM::new();
    // Load modules and execute bytecode
}
```

## License

Licensed under MIT OR Apache-2.0.
