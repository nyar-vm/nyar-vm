# nyar-aot

Ahead-of-Time (AOT) compiler for the Nyar VM.

## Overview

`nyar-aot` is the Ahead-of-Time compilation engine for the Nyar VM ecosystem. It leverages the power of E-Graphs and equality saturation to perform deep optimizations on Nyar's intermediate representation (IKun) before generating highly efficient native machine code.

## Key Features

- **E-Graph Based Optimization**: Uses `UniversalOptimizer` from the `chomsky` project to perform global, rule-based optimizations.
- **Equality Saturation**: Automatically explores thousands of equivalent code patterns to find the most efficient implementation based on a cost model.
- **Pluggable Backends**: Supports multiple target architectures and output formats (e.g., native binaries, static libraries) via the `Backend` abstraction.
- **IKun Integration**: Designed to work directly with the unified intermediate representation used by all Nyar frontends.
- **Cost-Model Driven Extraction**: Selects the best candidate from the E-Graph using sophisticated cost models tailored for different target architectures.

## Architecture

The AOT compiler follows a classic three-stage structure:
1. **Frontend**: Receives `IKun` (Gaia IR intent) from language frontends.
2. **Optimizer**: Loads the intent into an E-Graph, applies a set of rewrite rules, and runs saturation search.
3. **Backend**: Extracts the optimal tree and passes it to a specific backend (like `x86_64-assembler` or `pe-assembler`) to generate the final artifact.

## Usage

```rust
use nyar_aot::NyarAot;
use chomsky_extract::Backend;

fn main() {
    let mut aot = NyarAot::new();
    // Add intents (IKun) to the optimizer
    // ...
    // Compile using a specific backend
    // let artifact = aot.compile(&ikun, &my_backend).unwrap();
}
```

## License

Licensed under MIT OR Apache-2.0.
