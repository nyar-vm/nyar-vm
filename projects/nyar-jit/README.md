# nyar-jit

A multi-tier Just-In-Time (JIT) compiler for the Nyar VM.

## Overview

`nyar-jit` is a sophisticated JIT compilation engine that optimizes the execution of Gaia IR by compiling hot code paths into native machine instructions. It features a multi-tier architecture, advanced E-Graph based optimization, and support for dynamic runtime optimizations like Inline Caching and On-Stack Replacement.

## Key Features

- **Multi-Tier Compilation**:
  - **Tier 0 (Interpreter)**: Default execution mode for cold code.
  - **Tier 1 (Baseline)**: Fast compilation for initial warm-up, performing simple peephole optimizations.
  - **Tier 2 (Optimizing)**: Advanced optimizations using E-Graph equality saturation for hot code paths.
  - **Tier 3 (Extreme)**: Full-scale global optimization for the most critical loops and functions.
- **E-Graph Optimization Pipeline**:
  - Leverages the `chomsky` optimizer to perform rule-based optimizations.
  - **Equality Saturation**: Explores multiple optimization paths simultaneously, ensuring optimal code generation without fragile heuristic ordering.
- **On-Stack Replacement (OSR)**:
  - Dynamically transitions from the interpreter to JIT-compiled code during long-running loops.
  - Improves performance for long-running computations without waiting for function re-entry.
- **Dynamic Optimization & Inline Caching (IC)**:
  - Optimizes dynamic method calls and field accesses by caching lookup results directly at call sites.
  - Automatically deoptimizes if runtime assumptions (e.g., object shapes) are violated.
- **Advanced Optimization Rules**:
  - **Algebraic Simplification**: Constant folding, strength reduction, and identity elimination.
  - **Allocation Sinking**: Eliminates unnecessary object allocations by scalarizing objects or moving allocations out of hot loops.
  - **Barrier Elision**: Analyzes object lifetimes to remove redundant GC write barriers.
- **Tiering Thresholds**: Highly configurable hotness counters trigger transitions between different execution tiers based on invocation count and loop iterations.

## Architecture

- **`NyarJit`**: The primary `JitProvider` implementation that interfaces with the VM.
- **`UniversalOptimizer`**: The core optimization engine powered by `chomsky`, managing the E-Graph and rule applications.
- **`InlineCache`**: Manages a global registry of polymorphic and monomorphic inline caches for dynamic dispatch.
- **`CompiledCode`**: Represents a unit of native code with associated metadata for stack mapping, deoptimization points, and OSR entry.

## Execution Flow

1. **Profiling**: The interpreter tracks invocation and loop backedge counts.
2. **Threshold Trigger**: Once a threshold is reached, the function is scheduled for JIT compilation.
3. **UIR Lowering**: Gaia IR is converted to Universal Intermediate Representation (UIR).
4. **Optimization**: E-Graph saturation applies optimization rules.
5. **Code Generation**: The optimal tree is lowered to native machine code (x86_64, etc.) via `gaia-jit`.
6. **Installation**: The compiled code is installed into the VM's code cache.
7. **Deoptimization**: If a JIT assumption fails, the state is reconstructed and execution safely falls back to the interpreter.

## License

Licensed under MIT OR Apache-2.0.
