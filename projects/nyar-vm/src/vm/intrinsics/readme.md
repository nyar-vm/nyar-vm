# VM Intrinsics

This directory contains the built-in core operator implementations for the Nyar VM.

## Positioning

The `intrinsics` module is responsible for the **Flesh** and **Computation Logic** of the virtual machine. It defines the underlying operation rules for various data types.

## Key Responsibilities

- **Arithmetic Operations**: Implements basic operations for integers (`arith/i32.rs`, `i64.rs`) and floating-point numbers (`arith/float.rs`).
- **Complex Types**: Handles core operations for big integers (`bigint.rs`), strings (`string.rs`), and byte arrays (`bytes.rs`).
- **Semantic Unification**: Ensures consistency of computation results across platforms, such as the UTF-8 boundary checks enforced in `string.rs`.
- **Performance Optimization**: Provides direct call functions on hot paths, avoiding the overhead of calling external FFI via hash table lookups.

## Relationship with Operations

`intrinsics` focuses on **"How to compute specific operators"**. It is called by the `operations` module to provide the actual computational power for bytecode instructions. It is generally unaware of the VM's instruction flow or call stack structure, focusing solely on the transformation of the data itself.
