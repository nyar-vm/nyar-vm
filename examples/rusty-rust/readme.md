# rusty-rust

A simplified Rust-style language frontend for the Nyar virtual machine.

## Overview

This project implements a compiler for a simplified version of the Rust language. It is primarily used to verify Nyar's correctness when handling Rust-like syntax and compiling to managed environments such as the .NET CLR (Common Language Runtime).

## Features

### Language Features
- **Variable Management**: `let` declarations with support for mutability (`mut`).
- **Data Structures**: `struct` definitions, instantiation, and array literals.
- **Control Flow**: `if-else`, `while`, and range-based `for` loops (`for i in 0..10`).
- **Expressions**: 
  - Basic arithmetic and logical operations.
  - Function, method, and macro calls (e.g., `println!`).
  - Field and index access.
- **Type System**: Built-in types including `i32`, `i64`, `f32`, `f64`, `string`, and `bool`.

### Compilation Targets
Supports multiple targets via the `--target` flag:
- `il`: .NET IL (Intermediate Language)
- `clr`: .NET CLR binary
- `jvm`: Java Virtual Machine bytecode
- `pe`: Windows Portable Executable
- `wasi`: WebAssembly System Interface

## Getting Started

### Usage
```bash
# Compile Mini Rust source file
cargo run -- <input.vrs> --target clr
```

### CLI Arguments
- `-t, --target <TARGET>`: Compilation target (default: `all`).
- `-o, --output <DIR>`: Output directory (default: `target`).
- `-v, --verbose`: Enable verbose logging.

## Project Structure
- `src/ast/`: AST definitions and type conversion logic.
- `src/codegen/`: Code generator mapping AST to Gaia or target-specific instructions.

## License

Licensed under MIT OR Apache-2.0.
