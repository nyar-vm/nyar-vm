# nyar-gc

A high-performance, precise, and concurrent garbage collector for the Nyar VM.

## Overview

`nyar-gc` is a specialized garbage collection library designed for the unique requirements of the Nyar VM. It implements a precise mark-and-sweep algorithm with support for thread-local allocation and parallel marking, ensuring low latency and high throughput for managed objects.

## Key Features

- **Precise Mark-and-Sweep**: A robust collection strategy that correctly handles complex object graphs and cycles without the overhead of reference counting.
- **Block-Based Memory Management**:
  - Allocates memory in fixed-size 1MB blocks (`GcBlock`).
  - Reduces external fragmentation and simplifies sweep operations.
- **High-Speed TLAB (Thread-Local Allocation Buffer)**:
  - Fast, lock-free allocation for small objects within 64KB thread-local buffers.
  - Minimizes contention on the global allocator.
- **Size-Classed Free Lists**:
  - Efficiently manages memory holes of different sizes to minimize internal fragmentation.
  - Fast lookup for allocation requests.
- **Parallel Bitmapped Marking**:
  - Utilizes multiple threads for the mark phase, significantly reducing stop-the-world pauses.
  - Uses a compact mark bitmap within each block for high-speed object state checks.
- **Large Object Support**:
  - Dedicated handling for objects exceeding standard block sizes, ensuring efficient management of large arrays and buffers.
- **Integrated Write Barriers**:
  - Support for write barriers enables future implementation of incremental and generational collection strategies.
- **Trace Trait & Procedural Macros**:
  - Fully precise tracing via the `Trace` trait.
  - Automatic implementation using `#[derive(Trace)]` from `nyar-macros`.

## Architecture

- **`NyarGc`**: The central collector instance, managing the lifecycle of all blocks and coordinating collection cycles.
- **`GcBlock`**: The fundamental unit of memory, containing a header, mark bitmap, and object storage area.
- **`Tlab`**: Thread-local storage for fast allocation.
- **`Gc<T>` / `GcBox<T>`**: Smart pointers for managed objects, providing a safe and ergonomic interface for developers.
- **`MarkContext`**: Tracks the state of the current collection cycle, including the grey stack for parallel marking.

## Technical Specifications

| Parameter | Value |
|-----------|-------|
| Block Size | 1,024 KB |
| TLAB Size | 64 KB |
| Marking | Parallel Bitmapped |
| Algorithm | Mark-and-Sweep |
| Precision | Fully Precise |

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
