# rusty-python

A Python-like language frontend for the Nyar virtual machine.

## Overview

This project implements a compiler frontend for a subset of the Python language, designed to verify Nyar's support for dynamic language features and its ability to generate standard `.pyc` bytecode files.

## Features

### Language Features
- **Object-Oriented**: Support for `class` definitions and inheritance.
- **Functional Programming**: Support for `def` definitions and `lambda` expressions.
- **Control Flow**: Full `if-elif-else`, `for-in`, and `while` loops, including `break`, `continue`, and `pass`.
- **Exception Handling**: Support for `try-except-finally` blocks and `raise`.
- **Advanced Syntax**: List, dictionary, and set comprehensions.
- **Module System**: Support for `import` and `from ... import`.
- **Context Management**: Support for `with` statements.

### Compiler Features
- Generates Gaia Intermediate Representation (Gaia IR).
- **Core Feature**: Generates standard Python bytecode (`.pyc`) compatible with standard Python runtimes.

## Getting Started

### Usage
```bash
# Generate Python bytecode
cargo run -- <input.py> --pyc
```

### Options
- `--pyc`: Compile to a Python bytecode file.
- `--ast`: Output the Abstract Syntax Tree.
- `--tokens`: Output the lexer token stream.
- `--gaia`: Output Gaia instructions.
- `--gaia-json`: Output Gaia instructions in JSON format.

## Project Structure
- `src/ast.rs`: AST definitions covering rich Python syntax.
- `src/pyc_codegen.rs`: Backend implementation for generating `.pyc` files.

## License

Licensed under MIT OR Apache-2.0.
