use crate::NyarValue;
use std::ptr::NonNull;
use wasmtime::{component::__internal::StoreOpaque, ValRaw, ValType, WasmParams, WasmResults, WasmTy};
use wasmtime_runtime::{VMContext, VMNativeCallFunction, VMOpaqueContext};

unsafe impl WasmParams for NyarValue {
    type Abi = ();

    fn typecheck(params: impl ExactSizeIterator<Item = ValType>) -> wasmtime::Result<()> {
        todo!()
    }

    fn externrefs_count(&self) -> usize {
        todo!()
    }

    fn into_abi(self, store: &mut StoreOpaque) -> Option<Self::Abi> {
        todo!()
    }

    unsafe fn invoke<R: WasmResults>(
        func: NonNull<VMNativeCallFunction>,
        vmctx1: *mut VMOpaqueContext,
        vmctx2: *mut VMContext,
        abi: Self::Abi,
    ) -> R::ResultAbi {
        todo!()
    }
}

unsafe impl WasmResults for NyarValue {
    type ResultAbi = ();

    unsafe fn from_abi(store: &mut StoreOpaque, abi: Self::ResultAbi) -> Self {
        todo!()
    }
}
