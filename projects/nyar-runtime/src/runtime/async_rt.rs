use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::{Value, Frame};
use nyar_gc::Trace;
use nyar_types::{NyarError, NyarErrorKind, VmErrorKind};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

tokio::task_local! {
    /// Task-local storage for the current VM's traceback.
    /// This allows async tasks to report their VM-level call stack.
    pub static VM_TRACEBACK: String;
}

pub struct NyarRuntime {
    handle: tokio::runtime::Handle,
}

impl NyarRuntime {
    pub fn new() -> Self {
        Self {
            handle: tokio::runtime::Handle::current(),
        }
    }

    pub fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.handle.spawn(future)
    }

    pub fn spawn_vm(&self, mut vm: NyarVM, module_idx: usize, chunk_idx: usize) -> tokio::task::JoinHandle<Result<Value, NyarError>> {
        let gc = vm.gc.clone();
        self.handle.spawn(async move {
            // Register VM as a global root while it's running in an async task
            unsafe {
                gc.register_global_root(&vm as *const NyarVM as *const dyn Trace);
            }
            let future = VmFuture {
                vm: &mut vm,
                module_idx,
                chunk_idx,
            };
            let res = future.await;
            // Unregister VM from global roots after completion
            unsafe {
                gc.unregister_global_root(&vm as *const NyarVM as *const dyn Trace);
            }
            res
        })
    }
}


/// A wrapper for VM execution in an async context, integrating with nyar-gc's cooperative yielding.
pub struct VmFuture<'a> {
    pub vm: &'a mut NyarVM,
    pub module_idx: usize,
    pub chunk_idx: usize,
}

impl<'a> Future for VmFuture<'a> {
    type Output = Result<Value, NyarError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // If we are in a tokio task, update the traceback summary
        let summary = self.vm.get_traceback_summary();
        VM_TRACEBACK.sync_scope(summary, || self.poll_internal(cx))
    }
}

impl<'a> VmFuture<'a> {
    fn poll_internal(&mut self, cx: &mut Context<'_>) -> Poll<Result<Value, NyarError>> {
        // 0. Update current waker
        self.vm.current_waker = Some(cx.waker().clone());

        // 1. Check if GC requested a stop
        if nyar_gc::runtime::GC_STOP_THE_WORLD.load(std::sync::atomic::Ordering::Acquire) {
            // Cooperative yield for GC
            self.vm.gc.flush_thread_local();
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        // Register VM as root for the duration of this poll
        use nyar_gc::stack::StackRootGuard;
        let _vm_root = unsafe { StackRootGuard::<'static, NyarVM>::from_raw(self.vm as *const NyarVM) };

        // Set current GC for write barriers
        let _gc_guard = nyar_vm::vm::core::CURRENT_GC.with(|curr| {
            let mut curr = curr.borrow_mut();
            let old = curr.take();
            *curr = Some(self.vm.gc.clone());
            old
        });

        // 2. Execute a slice of instructions
        // We limit the number of instructions per poll to remain responsive to GC and other tasks
        let mut loop_count = 0;

        let res = loop {
            if loop_count >= 1024 {
                // 3. Not finished yet, yield and continue in next poll
                self.vm.gc.flush_thread_local();
                cx.waker().wake_by_ref();
                break Poll::Pending;
            }
            loop_count += 1;

            // Ensure we have at least one frame to execute
            if self.vm.frames.is_empty() {
                let instrs = match self.vm.get_chunk_instructions(self.module_idx, self.chunk_idx) {
                    Ok(i) => i,
                    Err(e) => {
                        self.vm.gc.flush_thread_local();
                        break Poll::Ready(Err(e));
                    }
                };
                self.vm.frames.push(Frame {
                    instrs,
                    upvalues: vec![None; 32],
                    module_idx: self.module_idx,
                    chunk_idx: Some(self.chunk_idx),
                    ip: 0,
                    locals: vec![Value::null(); 32],
                    location: Default::default(),
                    closure: Value::null(),
                });
            }

            match self.vm.execute_step() {
                Ok(Some(())) => {}
                Ok(None) => {
                    self.vm.gc.flush_thread_local();
                    break Poll::Ready(Ok(self.vm.pop().unwrap_or(Value::null())));
                }
                Err(e) if matches!(*e.kind, NyarErrorKind::Vm(VmErrorKind::YieldAsync)) => {
                    // Await on a pending future. The waker has been registered in execute_await.
                    // We don't wake_by_ref here because we wait for the IO/future to wake us.
                    self.vm.gc.flush_thread_local();
                    break Poll::Pending;
                }
                Err(e) => {
                    self.vm.gc.flush_thread_local();
                    break Poll::Ready(Err(e));
                }
            }
        };

        // Restore old GC
        nyar_vm::vm::core::CURRENT_GC.with(|curr| {
            *curr.borrow_mut() = _gc_guard;
        });

        res
    }
}
