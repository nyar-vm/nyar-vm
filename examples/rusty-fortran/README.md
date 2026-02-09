# rusty-fortran

A Fortran language frontend for the Nyar VM.

## Overview

`rusty-fortran` is a compiler frontend that brings the power of Fortran to the Nyar VM. It is designed for high-performance scientific and numerical computing, allowing legacy and modern Fortran code to run on a modern, JIT-optimized runtime with advanced memory management.

## Features

- **Numerical Performance**: Optimized for Fortran's specialized array operations and numerical types.
- **Modern Runtime**: Benefits from `nyar-vm`'s multi-tier JIT and `nyar-gc` for robust memory management.
- **Scientific Computing Integration**: Designed to interoperate with other Nyar-supported scientific languages like R and Julia.
- **Nyar Integration**: Compiles to Gaia IR, enabling global optimizations via `nyar-jit` and `nyar-aot`.

## Supported Constructs

- **Core Syntax**: `PROGRAM`, `SUBROUTINE`, `FUNCTION`, `MODULE`.
- **Data Types**: `INTEGER`, `REAL`, `COMPLEX`, `LOGICAL`, `CHARACTER`.
- **Arrays**: Support for multi-dimensional arrays and array slicing.
- **Control Flow**: `IF`, `DO`, `SELECT CASE`.
- **Modern Features**: Initial support for Fortran 90/95/2003 features like modules and derived types.

## Getting Started

### Usage via Nyar CLI

```bash
nyar run program.f90
```

## License

Licensed under MIT OR Apache-2.0.
