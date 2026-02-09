# nyar

The unified Command-Line Interface (CLI) for the Nyar VM.

## Overview

The `nyar` command is the primary entry point for interacting with the Nyar VM ecosystem. It provides a single, consistent interface for running scripts, compiling programs, and managing projects across dozens of supported programming languages.

## Installation

```bash
cargo install --path projects/nyar
```

## Key Commands

### `run`

Executes a source file directly. The CLI automatically detects the language based on the file extension and uses the appropriate frontend.

```bash
nyar run script.py
nyar run program.lua
nyar run main.go
```

### `compile`

Compiles a source file into a native binary or a Gaia IR module.

```bash
nyar compile script.py -o output_binary
```

### `repl`

Starts an interactive Read-Eval-Print Loop for a specific language.

```bash
nyar repl --lang python
nyar repl --lang lua
```

## Supported Languages

`nyar` supports a wide range of languages via its "Rusty" frontend family:
- **Scripting**: Python, Lua, Ruby, TCL, PHP
- **Systems**: C, Rust, Zig, Swift, Mojo
- **Managed**: Java, C#, Go, Dart, Kotlin
- **Data/Logic**: R, Julia, Prolog, SQL
- **Functional**: Scheme, Elixir, F#, Nim

## How it Works

The CLI uses `NyarDriver` to orchestrate the execution process:
1. **Frontend Selection**: Based on the file extension.
2. **Parsing & Lowering**: The frontend converts source code into Gaia IR.
3. **VM Execution**: The `nyar-vm` executes the IR, utilizing `nyar-gc` for memory management and `nyar-jit` for performance.

## License

Licensed under MIT OR Apache-2.0.
