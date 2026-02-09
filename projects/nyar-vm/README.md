# nyar-vm

The core execution engine for the Nyar virtual machine ecosystem.

## Overview

`nyar-vm` is a high-performance, stack-based virtual machine designed to execute Gaia Intermediate Representation (Gaia IR). It serves as the foundation for a wide range of language frontends, providing a robust and efficient runtime for both functional and object-oriented programming paradigms.

## Key Features

- **NaN-Boxing Value Representation**:
  - Efficiently stores integers, booleans, pointers, and small strings within a 64-bit double-precision floating-point format.
  - Minimizes memory footprint and improves cache locality.
  - Direct hardware support for floating-point operations.
- **Algebraic Effects & Delimited Continuations**:
  - Built-in support for first-class effect handlers.
  - Enables powerful control flow abstractions like `async/await`, generators, backtracking, and dependency injection without complex state machines.
- **Gaia IR Execution**:
  - A comprehensive instruction set designed for high-level language mapping.
  - Support for Object-Oriented (virtual calls, fields) and Functional (tail calls, closures) patterns.
  - Integrated metaprogramming primitives (quotes, splices).
- **Multi-Tier Execution Pipeline**:
  - Starts with an efficient bytecode interpreter for fast startup.
  - Seamlessly transitions to JIT-compiled native code via `nyar-jit` for hot paths.
- **Advanced Concurrency Model**:
  - Native support for lightweight tasks and futures.
  - Non-blocking I/O and event-driven execution built into the core.
- **Robust FFI System**:
  - Extensible registry for bridging with native Rust code and system libraries.
  - Support for both synchronous and asynchronous foreign function calls.

## Architecture

The VM is composed of several high-level modules:
- **`vm::core`**: The main execution engine, maintaining the stack, call frames, and effect handler stacks.
- **`vm::value`**: Implements the NaN-boxed value system and GC integration.
- **`vm::ops`**: Contains implementation of the Gaia IR instruction set, categorized by functionality (arithmetic, control, effects, etc.).
- **`bytecode`**: Handles loading, decoding, and encoding of the Gaia IR binary format.
- **`runtime`**: Provides platform-specific abstractions for I/O, networking, and filesystem access.

## Instruction Set Highlights

- **Arithmetic**: Typed operations for `i32`, `i64`, `f64`, and `BigInt`.
- **Object**: `GET_FIELD`, `SET_FIELD`, `INVOKE_VIRTUAL`, `INSTANCE_OF`.
- **Functional**: `MAKE_CLOSURE`, `TAIL_CALL`, `RETURN`.
- **Effects**: `HANDLE`, `PERFORM`, `RESUME`.
- **Metaprogramming**: `QUOTE`, `SPLICE`, `EVAL`.

## Getting Started

### Usage

To integrate the VM into your project:

```rust
use nyar_vm::NyarVM;
use std::sync::Arc;

fn main() {
    let vm = NyarVM::new();
    // Load Gaia IR modules and execute
    // let result = vm.execute_module(my_module);
}
```

## License

Licensed under MIT OR Apache-2.0.
