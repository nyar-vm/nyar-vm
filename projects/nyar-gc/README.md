# nyar-gc

A high-performance, precise, and concurrent garbage collector for the Nyar virtual machine.

## Overview

`nyar-gc` is a specialized garbage collection library designed for the unique requirements of the Nyar VM. It implements a precise mark-and-sweep algorithm with support for thread-local allocation and parallel marking, ensuring low latency and high throughput for managed objects.

## Key Features

- **Mark-and-Sweep Algorithm**: A robust, precise collection strategy that correctly handles complex object graphs and cycles.
- **Block-Based Memory Management**: Allocates memory in fixed-size 1MB blocks, reducing fragmentation and simplifying sweep operations.
- **TLAB (Thread-Local Allocation Buffer)**: Fast, lock-free allocation for small objects within 64KB thread-local buffers.
- **Size-Classed Free Lists**: Efficiently manages free memory holes of different sizes to minimize external fragmentation.
- **Parallel Marking**: Utilizes multiple threads to speed up the mark phase, significantly reducing stop-the-world pauses on multi-core systems.
- **Large Object Support**: Dedicated handling for large objects that exceed standard block sizes.
- **Bitmapped Marking**: Uses a compact mark bitmap for fast object state checks and efficient sweeping.
- **Write Barriers**: Built-in support for write barriers, enabling future implementation of incremental or generational collection.

## Technical Details

- **Block Size**: 1,024 KB
- **TLAB Size**: 64 KB
- **Marking Strategy**: Parallel bitmapped mark-and-sweep.
- **Precision**: Fully precise tracing using the `Trace` trait and procedural macros.

## Architecture

- **`NyarGc`**: The global collector instance managing blocks, free lists, and the collection lifecycle.
- **`GcBlock`**: The fundamental unit of memory allocation, containing its own mark bitmap and object metadata.
- **`Tlab`**: A thread-local structure for high-speed allocation.
- **`Gc<T>` / `GcBox<T>`**: Smart pointers for managed objects, providing seamless integration with the collector.
- **`Trace` Trait**: A core abstraction for object graph traversal, typically implemented via `#[derive(Trace)]`.

## Usage

### Defining Managed Types

```rust
use nyar_gc::{Trace, MarkContext};

#[derive(Trace)]
struct MyObject {
    other: Option<nyar_gc::Gc<MyObject>>,
    data: i32,
}
```

### Allocation

```rust
use nyar_gc::NyarGc;
use std::sync::Arc;

let gc = Arc::new(NyarGc::new());
let obj = gc.alloc(MyObject { other: None, data: 42 });
```

## License

Licensed under MIT OR Apache-2.0.
