use crate::vm::core::NyarVM;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct AsyncRuntime {}

impl AsyncRuntime {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A wrapper for VM execution in an async context, integrating with nyar-gc's cooperative yielding.
pub struct VmFuture<'a> {
    pub vm: &'a mut NyarVM,
    pub module_idx: usize,
    pub chunk_idx: usize,
}

impl<'a> Future for VmFuture<'a> {
    type Output = Result<crate::vm::value::Value, crate::vm::VmError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
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
        let _vm_root = StackRootGuard::new(self.vm);
        
        while loop_count < 1024 {
            loop_count += 1;
            
            let (cur_ip, module_idx) = {
                let f = match self.vm.frames.last() {
                    Some(f) => f,
                    None => return Poll::Ready(self.vm.pop()),
                };
                if f.ip >= f.instrs.len() {
                    return Poll::Ready(self.vm.pop());
                }
                (f.ip, f.module_idx)
            };

            let ins = self.vm.frames.last().unwrap().instrs[cur_ip].clone();
            match self.vm.dispatch_instruction(ins, cur_ip, module_idx) {
                Ok(next_ip) => {
                    if let Some(f) = self.vm.frames.last_mut() {
                        if let Some(new_ip) = next_ip {
                            f.ip = new_ip;
                        } else {
                            f.ip += 1;
                        }
                    } else {
                        return Poll::Ready(self.vm.pop());
                    }
                }
                Err(e) => return Poll::Ready(Err(e)),
            }
        }

        // 3. Not finished yet, yield and continue in next poll
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}
