# nyar-macros

Procedural macros for the Nyar virtual machine ecosystem.

## Overview

`nyar-macros` provides convenient procedural macros that automate repetitive tasks and boilerplate code in the Nyar project. Its primary focus is on ensuring correct interaction with the garbage collector and the type system.

## Available Macros

### `#[derive(Trace)]`

The most important macro in the project. It automatically implements the `Trace` trait for any struct or enum. The `Trace` trait is used by `nyar-gc` to traverse the object graph during the mark phase.

**Features**:
- **Automatic Field Traversal**: Recursively calls `.trace(ctx)` on all fields of a struct.
- **Enum Support**: Correctly handles all variants of an enum, including named, unnamed, and unit variants.
- **Smart Pointer Awareness**: Works seamlessly with `Gc<T>`, `Option<T>`, `Vec<T>`, and other common wrappers that implement `Trace`.

**Example**:

```rust
use nyar_gc::{Trace, Gc};
use nyar_macros::Trace;

#[derive(Trace)]
struct Node {
    value: i32,
    next: Option<Gc<Node>>,
    children: Vec<Gc<Node>>,
}
```

## How it Works

The macros use `syn` and `quote` to parse Rust ASTs and generate implementation blocks at compile time. This ensures type safety and zero runtime overhead for the implementations themselves.

## License

Licensed under MIT OR Apache-2.0.
