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

#[derive(Default)]
pub struct AsyncRuntime {}

impl AsyncRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// Blocks the current thread until the given future completes.
    /// This is a simple executor that doesn't require an external async runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        let mut future = Box::pin(future);
        let waker = self.create_waker();
        let mut cx = Context::from_waker(&waker);
        loop {
            match future.as_mut().poll(&mut cx) {
                Poll::Ready(res) => return res,
                Poll::Pending => {
                    std::thread::yield_now();
                }
            }
        }
    }

    fn create_waker(&self) -> std::task::Waker {
        use std::task::{RawWaker, RawWakerVTable, Waker};

        unsafe fn noop(_: *const ()) {}
        unsafe fn clone(p: *const ()) -> RawWaker {
            RawWaker::new(p, &VTABLE)
        }

        static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);
        unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
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
            return VM_TRACEBACK.scope(summary, || self.poll_internal(cx));
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
            
            match self.vm.execute_step() {
                Ok(Some(())) => {}
                Ok(None) => return Poll::Ready(Ok(self.vm.pop().unwrap_or(crate::vm::value::Value::null()))),
                Err(e) if matches!(*e.kind, nyar_types::NyarErrorKind::Vm(nyar_types::VmErrorKind::YieldAsync)) => {
                    // Instruction requested a yield
                    cx.waker().wake_by_ref();
                    return Poll::Pending;
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        // 3. Not finished yet, yield and continue in next poll
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
