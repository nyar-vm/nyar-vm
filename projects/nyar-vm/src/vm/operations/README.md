# VM Operations

This directory contains the core execution logic for the Nyar VM instruction set.

## Positioning

The `operations` module is responsible for the **Skeleton** and **Control Flow** of the virtual machine. It defines how the VM interprets bytecode, manages the call stack, and handles instruction dispatch.

## Key Responsibilities

- **Instruction Dispatch**: Maps bytecode to specific execution functions in `dispatch_instruction` within `mod.rs`.
- **Control Flow**: Handles jumps (`control.rs`), function calls (`call.rs`), and returns.
- **Stack Management**: Manages the operand stack, including pushing, popping, and variable access (`stack.rs`).
- **Advanced Features**: Implements instructions for closures (`closure.rs`), algebraic effects (`effects.rs`), and metaprogramming (`metaprogramming.rs`).

## Relationship with Intrinsics

`operations` focuses on **"How to execute instructions"**. When it comes to the actual computation logic for specific data types (such as `I32Add` or `StringSubstr`), it calls upon the concrete implementations within the `intrinsics` module.
