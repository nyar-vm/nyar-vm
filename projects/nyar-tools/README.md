# nyar-tools

Diagnostic and development utilities for the Nyar VM.

## Overview

`nyar-tools` provides a suite of tools for developers working on the Nyar VM or language frontends. It includes utilities for bytecode disassembly, IR visualization, and runtime profiling.

## Key Features

- **Bytecode Disassembler**: Converts binary Gaia IR modules into human-readable assembly format, showing instructions, constant pools, and metadata.
- **IR Visualizer**: Generates Graphviz DOT files or interactive visualizations of the E-Graph and optimization passes.
- **Heap Profiler**: Tools for inspecting the GC heap, identifying memory leaks, and visualizing object distribution.
- **Benchmark Suite**: A collection of standard benchmarks to measure VM performance across different language frontends.
- **Testing Infrastructure**: Shared utilities for writing integration tests that span multiple language frontends and the VM.

## Usage

Many of these tools are integrated into the main `nyar` CLI but can also be used as standalone libraries for custom tooling.

```rust
use nyar_tools::disassembler::Disassembler;

fn main() {
    let module = load_module("program.nyarc");
    let asm = Disassembler::new().disassemble(&module);
    println!("{}", asm);
}
```

## License

Licensed under MIT OR Apache-2.0.
