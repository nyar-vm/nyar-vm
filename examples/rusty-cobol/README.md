# rusty-cobol

A COBOL language frontend for the Nyar virtual machine.

## Overview

`rusty-cobol` is a specialized compiler frontend that brings the venerable COBOL language to the modern Nyar virtual machine ecosystem. It allows legacy business logic and data processing applications to run on a high-performance, JIT-optimized runtime with advanced garbage collection and concurrency support.

## Features

- **Standard COBOL Support**: Aims to support common COBOL dialects and standards.
- **Modern Execution Environment**: Runs COBOL code on the `nyar-vm`, providing benefits like automatic memory management (via `nyar-gc`) and multi-tier JIT optimization.
- **Data Division Mapping**: Maps COBOL's complex data structures and pictures to Nyar's native types and objects.
- **Nyar Integration**: Seamlessly compiles to Gaia IR, enabling interoperability with modern languages like Python and Go.
- **High Performance**: Leverages `nyar-jit` to optimize critical business logic paths.

## Supported Constructs

- **Divisions**: Identification, Environment, Data, and Procedure divisions.
- **Data Types**: Alphanumeric, Numeric (including fixed-point decimals), and Group items.
- **Control Flow**: `PERFORM`, `IF`, `EVALUATE`, `GO TO`.
- **File I/O**: Initial support for standard COBOL file handling operations.

## Getting Started

### Usage via Nyar CLI

```bash
nyar run program.cbl
```

## License

Licensed under MIT OR Apache-2.0.
