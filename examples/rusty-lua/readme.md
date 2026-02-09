# rusty-lua

A Lua-like language frontend for the Nyar virtual machine.

## Overview

This project implements a compiler frontend for a subset of the Lua language, designed to verify Nyar's support for dynamic language features and its ability to generate Gaia-compliant instructions.

## Features

### Language Features
- **Basic Types**: Numbers, strings, booleans, and nil.
- **Tables**: Support for Lua's core data structure.
- **Functions**: Function definitions, anonymous functions, and closures.
- **Control Flow**: `if-then-else`, `for`, `while`, and `repeat-until` loops.
- **Local Variables**: Support for the `local` keyword.

### Compiler Features
- Generates Gaia Intermediate Representation (Gaia IR).
- Supports AST and Token stream dumping for debugging.

## Getting Started

### Usage
```bash
# View Abstract Syntax Tree
cargo run -- <input.lua> --ast

# Generate Gaia instructions
cargo run -- <input.lua> --gaia
```

### Options
- `--ast`: Output the Abstract Syntax Tree.
- `--tokens`: Output the lexer token stream.
- `--gaia`: Output Gaia instructions.
- `--gaia-json`: Output Gaia instructions in JSON format.

## Project Structure
- `src/lib.rs`: Core implementation of the Mini Lua frontend.
- `src/codegen.rs`: Gaia instruction generator.

## License

Licensed under MIT OR Apache-2.0.
