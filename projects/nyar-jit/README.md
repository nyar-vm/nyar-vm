# nyar-jit

A multi-tier Just-In-Time (JIT) compiler for the Nyar virtual machine.

## Overview

`nyar-jit` is a sophisticated JIT compilation engine that optimizes the execution of Gaia IR by compiling hot code paths into native machine instructions. It features a multi-tier architecture, advanced E-Graph based optimization, and support for dynamic runtime optimizations like Inline Caching and On-Stack Replacement.

## Key Features

- **Multi-Tier Compilation**:
  - **Baseline Tier**: Fast compilation for initial warm-up.
  - **Optimizing Tier**: Advanced optimizations using E-Graph equality saturation.
  - **Extreme Tier**: Full-scale optimization for the hottest code paths.
- **E-Graph Optimization**: Leverages the `chomsky` optimizer to perform global, rule-based optimizations, ensuring high-quality native code.
- **On-Stack Replacement (OSR)**: Ability to transition from the interpreter to JIT-compiled code even while a function is currently executing on the stack.
- **Inline Caching (IC)**: Optimizes dynamic method calls and field accesses by caching lookup results at call sites.
- **Advanced Optimization Rules**:
  - **Algebraic Simplification**: Simplifies complex arithmetic and logic expressions.
  - **Constant Folding**: Evaluates constant expressions at compile time.
  - **Allocation Sinking**: Eliminates unnecessary object allocations by moving them to where they are actually used or scalarizing them.
  - **Barrier Elision**: Removes redundant GC write barriers for performance.
- **Tiering Thresholds**: Configurable hotness thresholds to trigger transitions between different execution tiers.
- **Executable Memory Management**: Safe and efficient management of executable memory regions via `gaia-jit`.

## Architecture

- **`NyarJit`**: The primary `JitProvider` implementation for `nyar-vm`.
- **`UniversalOptimizer`**: The core optimization engine powered by `chomsky`.
- **`InlineCache`**: Manages runtime caches for dynamic dispatch.
- **`CompiledCode`**: Represents a unit of JIT-compiled native code with associated metadata like tier and OSR points.

## How it Works

1. **Profiling**: The interpreter tracks function "hotness" during execution.
2. **Threshold Trigger**: When a function exceeds a threshold, it is queued for JIT compilation.
3. **Compilation**: The Gaia IR is converted to an intermediate representation (UIR), optimized, and then lowered to native machine code (x86_64, etc.).
4. **Execution**: The VM's execution flow is diverted to the native entry point.
5. **Deoptimization**: If assumptions made during JIT (e.g., type stability) are violated, the execution safely falls back to the interpreter.

## License

Licensed under MIT OR Apache-2.0.
