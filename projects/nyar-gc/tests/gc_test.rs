use nyar_gc::{NyarGc, Trace, Gc, MarkContext, GcCell, Tlab};
use std::thread;
use std::sync::Arc;

#[test]
fn test_tlab_basic() {
    let gc = Arc::new(NyarGc::new());
    let mut tlab = Tlab::new(&gc);

    let node1 = tlab.alloc(TestNode {
        value: 1,
        next: GcCell::new(None),
    });

    assert_eq!(node1.value, 1);
}

#[test]
fn test_tlab_multithreaded() {
    let gc = Arc::new(NyarGc::new());
    let mut handles = Vec::new();

    for i in 0..4 {
        let gc_clone = gc.clone();
        handles.push(thread::spawn(move || {
            let mut tlab = Tlab::new(&gc_clone);
            for j in 0..1000 {
                let _node = tlab.alloc(TestNode {
                    value: i * 1000 + j,
                    next: GcCell::new(None),
                });
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Trigger GC to ensure everything is fine
    unsafe {
        gc.collect(|_| {});
    }
}

struct TestNode {
    value: i32,
    next: GcCell<Option<Gc<TestNode>>>,
}

impl Trace for TestNode {
    fn trace(&self, ctx: &mut MarkContext) {
        unsafe {
            if let Some(next) = self.next.get_ref() {
                next.trace(ctx);
            }
        }
    }
}

#[test]
fn test_gc_basic() {
    let gc = NyarGc::new();
    
    let node1 = gc.alloc(TestNode {
        value: 1,
        next: GcCell::new(None),
    });
    
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: GcCell::new(Some(node1)),
    });
    
    // Both should be reachable if we start from node2
    unsafe {
        gc.collect(|ctx| {
            node2.trace(ctx);
        });
    }
    
    assert_eq!(node2.value, 2);
    unsafe {
        assert_eq!(node2.next.get_ref().unwrap().value, 1);
    }
}

#[test]
fn test_gc_collect() {
    let gc = NyarGc::new();
    
    {
        let _node1 = gc.alloc(TestNode {
            value: 1,
            next: GcCell::new(None),
        });
    }
    // node1 is out of scope, but GC doesn't know that yet.
    
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: GcCell::new(None),
    });
    
    unsafe {
        gc.collect(|ctx| {
            node2.trace(ctx);
        });
    }
    
    assert_eq!(node2.value, 2);
}

#[test]
fn test_generational_gc() {
    let gc = NyarGc::new();

    // 1. Allocate a node (Young Gen)
    let node1 = gc.alloc(TestNode {
        value: 1,
        next: GcCell::new(None),
    });

    // 2. Perform minor GC, node1 survives and should be promoted to Old Gen
    unsafe {
        gc.collect_minor(|ctx| {
            node1.trace(ctx);
        });
    }

    // 3. Allocate another node (Young Gen)
    let node2 = gc.alloc(TestNode {
        value: 2,
        next: GcCell::new(None),
    });

    // 4. Update node1 (Old) to point to node2 (Young) -> Automated Write Barrier
    gc.write(node1, &node1.next, Some(node2));

    // 5. Perform minor GC. node2 should be reachable via node1 (card table)
    unsafe {
        gc.collect_minor(|ctx| {
            node1.trace(ctx);
        });
    }

    // 6. Verify node2 still exists
    unsafe {
        assert_eq!(node1.next.get_ref().unwrap().value, 2);
    }
}

#[test]
fn test_incremental_gc() {
    let gc = NyarGc::new();
    let root = gc.alloc(TestNode {
        value: 1,
        next: GcCell::new(None),
    });

    // Allocate many objects to trigger GC
    for i in 0..100 {
        let node = gc.alloc(TestNode {
            value: i,
            next: GcCell::new(None),
        });
        gc.write(root, &root.next, Some(node));

        // Perform small steps of GC
        unsafe {
            gc.step(5, |ctx| {
                root.trace(ctx);
            });
        }
    }

    // After many steps, GC should eventually finish a cycle
    assert!(root.value == 1);
}
