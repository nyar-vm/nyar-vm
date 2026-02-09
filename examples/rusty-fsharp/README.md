# rusty-fsharp

An F# language frontend for the Nyar VM.

## Overview

`rusty-fsharp` is a compiler frontend that brings the F# programming language to the Nyar VM. It combines F#'s concise syntax and functional-first approach with Nyar's advanced runtime, mapping F#'s unique features to the VM's native execution engine.

## Features

- **Functional-First**: Full support for F#'s functional programming paradigms, including immutability and pattern matching.
- **Algebraic Effects Integration**: Maps F#'s computation expressions and `async` workflows to Nyar's native algebraic effects.
- **Strongly Typed**: Maintains F#'s robust type system and type inference during the lowering process to Gaia IR.
- **Managed Execution**: Benefits from `nyar-gc` for automatic memory management and `nyar-jit` for performance.
- **Interoperability**: Seamlessly interacts with other languages in the Nyar ecosystem, such as C# and Python.

## Supported Constructs

- **Core Syntax**: `let` bindings, `type` definitions, `module`, `open`.
- **Functional Features**: Discriminated unions, record types, function currying, and piping (`|>`).
- **Pattern Matching**: Robust support for complex pattern matching in `match` and `let` expressions.
- **Computation Expressions**: Support for custom and built-in computation expressions.
- **Async Workflows**: Native support for asynchronous and concurrent programming.

## Getting Started

### Usage via Nyar CLI

```bash
nyar run script.fs
```

### Usage as a Library

```rust
use rusty_fsharp::RustyFSharpFrontend;
use nyar_types::NyarFrontend;

let frontend = RustyFSharpFrontend::new();
let ast = frontend.parse("let hello name = printfn \"Hello, %s!\" name\nhello \"F#\"").unwrap();
// Lower and execute via NyarVM
```

## License

Licensed under MIT OR Apache-2.0.
