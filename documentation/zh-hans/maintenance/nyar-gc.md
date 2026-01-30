# nyar-gc

`nyar-gc` 提供了一个专为 Nyar VM 优化的垃圾回收器。目前采用的是经典的 **Mark-and-Sweep（标记-清除）** 算法。

## 核心职责

- **安全指针 (`Gc<T>`)**: 提供自动内存管理的引用类型。
- **可达性分析**: 通过 `Trace` trait 遍历托管堆。
- **内存生命周期管理**: 自动分配与回收不再使用的对象。

## 内存布局

每一个由 GC 管理的对象都封装在一个 `GcBox<T>` 中，其结构如下：

```rust
#[repr(C)]
struct GcBox<T: Trace> {
    header: GcHeader, // 包含标记位、链表指针、析构函数
    data: T,          // 实际的用户数据
}
```

所有的 `GcHeader` 被组织成一个单向链表，由 `NyarGc` 持有头指针。这种设计允许 GC 遍历系统中所有的分配，而不依赖于特定的堆内存布局。

## 回收算法流程

1.  **Marking Phase (标记阶段)**:
    -   从根集合（Roots，如全局变量、栈上变量）出发。
    -   调用 `Trace::trace()` 递归标记所有可达的对象。
    -   使用 `Cell<bool>` 在 `GcHeader` 中记录标记状态。
2.  **Sweeping Phase (清除阶段)**:
    -   遍历 `NyarGc` 的全局对象链表。
    -   如果对象未被标记，则将其从链表中移除并调用析构函数释放内存。
    -   如果对象已被标记，则重置标记位以供下一轮使用。

## 开发者指南

### 实现 Trace Trait

对于任何包含 `Gc<T>` 的自定义结构，必须实现 `Trace` 以确保 GC 能正确发现嵌套的引用：

```rust
impl Trace for MyStruct {
    fn trace(&self) {
        self.some_gc_ptr.trace();
        self.other_data.trace();
    }
}
```

### 性能考量

- **Threshold**: `NyarGc` 会维护一个分配阈值（默认 1MB），当已分配字节超过阈值时自动触发 `collect`。
- **Stop-the-world**: 目前的实现在回收时需要暂停所有解释器线程以确保一致性。
