# Backend Maintenance Guide

This document introduces the compilation flow and design considerations for various backends supported by the Valkyrie compiler.

## Overview

Valkyrie supports multiple backends, each with its own characteristics and optimization strategies. The general pipeline is:
`Source -> AST -> HIR -> CFG -> SSA -> LIR -> Target`

However, depending on the target architecture (stack-based vs. register-based), certain stages may be skipped or modified.

## Backend Implementations

- [JVM Backend](jvm.md): Targets the Java Virtual Machine, using a stack-based architecture.
- [WASM Backend](wasm.md): Targets WebAssembly, using a stack-based architecture.
- [CLR Backend](clr.md): Targets the .NET runtime, using a stack-based architecture.
- [Native Backend](native.md): Evaluation scheme for native instruction sets (x86, x64, arm64, riscv).
- [ValkyrieVM](valkyrie-vm.md): Reference interpreter, using a register-based architecture.

## Design Decisions

### Skipping LIR Phase for Stack Machines

As of 2026-01-10, we have decided to skip the LIR phase for backends targeting stack machines (e.g., JVM).

**Rationale**:
- **Redundancy**: LIR is designed for register machines. Mapping SSA to LIR involves register allocation, which is unnecessary for stack machines.
- **Performance**: Generating stack-based instructions directly from CFG (which still preserves high-level expression information) allows for more natural utilization of the operand stack and avoids excessive local variable access.
