# rusty-julia

A Julia language frontend for the Nyar VM.

## Overview

`rusty-julia` is a compiler frontend that brings the Julia programming language to the Nyar VM. It focuses on Julia's strengths—high-performance numerical computing and multiple dispatch—mapping them to Nyar's native execution engine and optimization pipeline.

## Features

- **Multiple Dispatch**: Implements Julia's core multiple dispatch mechanism using Nyar's witness tables and virtual call system.
- **Numerical Performance**: Optimized for Julia's specialized array operations and numerical types.
- **Dynamic Typing with Static Performance**: Leverages `nyar-jit` and E-Graph optimization to achieve near-native performance for hot Julia code.
- **Metaprogramming**: Integrated with Nyar's macro system to support Julia's powerful macro and reflection capabilities.
- **Nyar Integration**: Compiles to Gaia IR, enabling interoperability with other scientific languages like R and Fortran.

## Supported Constructs

- **Core Syntax**: `function`, `struct`, `mutable struct`, `module`, `using`.
- **Multiple Dispatch**: Full support for defining methods with different type signatures.
- **Control Flow**: `if`, `for`, `while`, `try-catch`.
- **Arrays**: Robust support for Julia's multi-dimensional array system.
- **Macros**: Support for Julia-style macros and code generation.

## Getting Started

### Usage via Nyar CLI

```bash
nyar run script.jl
```

### Usage as a Library

```rust
use rusty_julia::RustyJuliaFrontend;
use nyar_types::NyarFrontend;

let frontend = RustyJuliaFrontend::new();
let ast = frontend.parse("function hello(name)\n    println(\"Hello, $name!\")\nend\nhello(\"Julia\")").unwrap();
// Lower and execute via NyarVM
```

## License

Licensed under MIT OR Apache-2.0.
