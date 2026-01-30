use nyar_gc::{NyarGc, Trace, Gc};
use std::cell::Cell;

struct TestNode {
    value: i32,
    next: Cell<Option<Gc<TestNode>>>,
}

impl Trace for TestNode {
    fn trace(&self) {
        if let Some(next) = self.next.get() {
            next.trace();
        }
    }
}

#[test]
fn test_gc_basic() {
    let gc = NyarGc::new();
    
    let node1 = gc.alloc(TestNode {
        value: 1,
        next: Cell::new(None),
    });
    
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: Cell::new(Some(node1)),
    });
    
    // Both should be reachable if we start from node2
    unsafe {
        gc.collect(|| {
            node2.trace();
        });
    }
    
    assert_eq!(node2.value, 2);
    assert_eq!(node2.next.get().unwrap().value, 1);
}

#[test]
fn test_gc_collect() {
    let gc = NyarGc::new();
    
    {
        let _node1 = gc.alloc(TestNode {
            value: 1,
            next: Cell::new(None),
        });
    }
    // node1 is out of scope, but GC doesn't know that yet.
    // In a real VM, roots would be on the stack or in registers.
    
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: Cell::new(None),
    });
    
    unsafe {
        gc.collect(|| {
            node2.trace();
        });
    }
    
    // After collection, node1 should be freed (though we can't easily check that without a custom allocator or tracking)
    assert_eq!(node2.value, 2);
}

#[test]
fn test_generational_gc() {
    let gc = NyarGc::new();
    
    // 1. Allocate a node (Young Gen)
    let node1 = gc.alloc(TestNode {
        value: 1,
        next: Cell::new(None),
    });
    
    // 2. Perform minor GC, node1 survives and should be promoted to Old Gen
    unsafe {
        gc.collect_minor(|| {
            node1.trace();
        });
    }
    
    // 3. Allocate another node (Young Gen)
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: Cell::new(None),
    });
    
    // 4. Update node1 (Old) to point to node2 (Young) -> Write Barrier
    node1.next.set(Some(node2));
    gc.write_barrier(node1, node2);
    
    // 5. Perform minor GC. node2 should be reachable via node1 (remembered set)
    unsafe {
        gc.collect_minor(|| {
            // No roots here, node2 should be found via node1 in remembered set
        });
    }
    
    // 6. Verify node2 still exists
    assert_eq!(node1.next.get().unwrap().value, 2);
}
