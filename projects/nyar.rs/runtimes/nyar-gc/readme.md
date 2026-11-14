# nyar-gc

Language-neutral managed heap and garbage collector used by OdinVM through explicit allocation, layout, root and write-barrier contracts. This crate does not define a shared Nyar bytecode or a mandatory runtime for other targets.

## Overview

`nyar-gc` provides allocation, tracing, collection, and tuning hooks used by `odin-vm::Executor`. It is
**language-agnostic**: it never imports `rusty-<lang>` or guest frontends. Object shapes arrive as `LayoutDescriptor`
values registered by the loader; the heap stores dense slot vectors keyed by `layout_id`.

## Features

- **`ObjectHeap`** — free-list allocation, layout table, write barrier, host roots, generation tags, live/total byte
  accounting.
- **`GarbageCollector`** — mark-sweep baseline with nursery / tenured soft capacities and promotion failure reporting.
- **`LayoutDescriptor` / `LayoutId`** — field-slot layout contract (no field names on the hot path).
- **`Value` / `ObjectPayload`** — NaN-boxed scalars plus `LayoutObject` and suspended `Coroutine` payloads.
- **`GcPolicy` / `WorkloadHints` / `WorkloadIntent`** — policy inputs and workload-driven strategy transitions.
- **`HostRoots` / `RootHandle`** — persistent roots supplied by the embedder.
- **`ConcurrentMarkController` / `WriteBarrier`** — concurrent marking protocol skeleton and SATB barrier hooks.
- **`StrategyController`** — bridges intent JSON into collector decisions.

## Integration with `odin-vm`

The VM embeds `ObjectHeap` and `GarbageCollector` inside `Executor`. During interpretation, stack slots and global
tables hold `Value` references; collection traces from roots through object graphs. Deoptimization paths use
`encode_value_for_deopt` helpers exported from `odin-vm`, not from this crate.

## Usage

```rust
use nyar_gc::{ObjectHeap, GarbageCollector, GcPolicy};

let mut heap = ObjectHeap::new();
let mut gc = GarbageCollector::new();
// Register layouts, allocate objects, trace from roots — see heap/collector APIs.
```

## Boundaries

- Does not load `.odin` files or decode bytecode.
- Does not define language type systems or witness tables. Their semantics belong to the corresponding language, while target runtime adapters supply explicit heap layout and ownership contracts.
