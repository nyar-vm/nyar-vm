use crate::vm::core::NyarVM;
use nyar_types::NyarError;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(feature = "tokio")]
tokio::task_local! {
    /// Task-local storage for the current VM's traceback.
    /// This allows async tasks to report their VM-level call stack.
    pub static VM_TRACEBACK: String;
}

#[cfg(feature = "tokio")]
pub struct NyarRuntime {
    handle: tokio::runtime::Handle,
}

#[cfg(feature = "tokio")]
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

    pub fn spawn_vm(&self, mut vm: NyarVM, module_idx: usize, chunk_idx: usize) -> tokio::task::JoinHandle<Result<crate::vm::value::Value, NyarError>> {
        self.handle.spawn(async move {
            let future = VmFuture {
                vm: &mut vm,
                module_idx,
                chunk_idx,
            };
            future.await
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
    type Output = Result<crate::vm::value::Value, NyarError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(feature = "tokio")]
        {
            // If we are in a tokio task, update the traceback summary
            let summary = self.vm.get_traceback_summary();
            return VM_TRACEBACK.sync_scope(summary, || self.poll_internal(cx));
        }

        #[cfg(not(feature = "tokio"))]
        self.poll_internal(cx)
    }
}

impl<'a> VmFuture<'a> {
    fn poll_internal(&mut self, cx: &mut Context<'_>) -> Poll<Result<crate::vm::value::Value, NyarError>> {
        // 0. Update current waker
        self.vm.current_waker = Some(cx.waker().clone());

        // 1. Check if GC requested a stop
        if nyar_gc::runtime::GC_STOP_THE_WORLD.load(std::sync::atomic::Ordering::Acquire) {
            // Cooperative yield for GC
            self.vm.gc.flush_thread_local();
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }

        // 2. Execute a slice of instructions
        // We limit the number of instructions per poll to remain responsive to GC and other tasks
        let mut loop_count = 0;
        
        // Register VM as root for the duration of this poll
        use nyar_gc::stack::StackRootGuard;
        let _vm_root = unsafe { StackRootGuard::<'static, NyarVM>::from_raw(self.vm as *const NyarVM) };
        
        while loop_count < 1024 {
            loop_count += 1;
            
            // Ensure we have at least one frame to execute
            if self.vm.frames.is_empty() {
                let instrs = match self.vm.get_chunk_instructions(self.module_idx, self.chunk_idx) {
                    Ok(i) => i,
                    Err(e) => return Poll::Ready(Err(e)),
                };
                self.vm.frames.push(crate::vm::value::Frame {
                    instrs,
                    upvalues: vec![None; 32],
                    module_idx: self.module_idx,
                    chunk_idx: Some(self.chunk_idx),
                    ip: 0,
                    locals: vec![crate::vm::value::Value::null(); 32],
                    location: Default::default(),
                    closure: crate::vm::value::Value::null(),
                });
            }

            match self.vm.execute_step() {
                Ok(Some(())) => {}
                Ok(None) => {
                    self.vm.gc.flush_thread_local();
                    return Poll::Ready(Ok(self.vm.pop().unwrap_or(crate::vm::value::Value::null())));
                }
                Err(e) if matches!(*e.kind, nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::YieldAsync)) => {
                    // Await on a pending future. The waker has been registered in execute_await.
                    // We don't wake_by_ref here because we wait for the IO/future to wake us.
                    self.vm.gc.flush_thread_local();
                    return Poll::Pending;
                }
                Err(e) => {
                    self.vm.gc.flush_thread_local();
                    return Poll::Ready(Err(e));
                }
            }
        }

        // 3. Not finished yet, yield and continue in next poll
        self.vm.gc.flush_thread_local();
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
