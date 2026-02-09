# rusty-dart

A Dart language frontend for the Nyar VM.

## Overview

`rusty-dart` is a compiler frontend that brings the Dart programming language to the Nyar VM. It is designed to provide a high-performance runtime for Dart applications, leveraging Nyar's advanced JIT compilation and garbage collection for both mobile and server-side scenarios.

## Features

- **Modern Dart Support**: Targeted at modern Dart specifications, including null safety.
- **Efficient Object System**: Maps Dart's class-based object model and mixins to Nyar's native object system.
- **Async/Await Integration**: Maps Dart's `Future` and `Stream` abstractions to Nyar's native algebraic effects and continuations.
- **High Performance**: Benefit from `nyar-jit` optimizations for hot Dart code.
- **Hot Reload Capability**: Designed to support future hot-reload features in the Nyar ecosystem.

## Supported Constructs

- **Core Syntax**: `class`, `mixin`, `extension`, `enum`.
- **Functions**: Support for closures, arrow functions, and optional parameters.
- **Collections**: Integrated support for Dart Lists, Sets, and Maps.
- **Async Programming**: `async`, `await`, `yield`.
- **Type System**: Support for Dart's sound type system and null safety checks.

## Getting Started

### Usage via Nyar CLI

```bash
nyar run main.dart
```

### Usage as a Library

```rust
use rusty_dart::RustyDartFrontend;
use nyar_types::NyarFrontend;

let frontend = RustyDartFrontend::new();
let ast = frontend.parse("void main() { print('Hello from Dart on Nyar!'); }").unwrap();
// Lower and execute via NyarVM
```

## License

Licensed under MIT OR Apache-2.0.
