use miette::{Result, miette};
use nyar::{BackendCandidate, BackendSelector, PartitionBackendRequirement};

use crate::{DriverCompileReport, DriverCompileRequest};

#[cfg(feature = "nyar-vm-lane")]
#[doc(hidden)]
pub mod nyar_vm;
mod wasm;

trait BundledBackendCompiler: Sync {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport>;
}

struct DriverCompilerRegistration {
    name: &'static str,
    priority: u16,
    supports: fn(&PartitionBackendRequirement) -> bool,
    compiler: &'static dyn BundledBackendCompiler,
}

impl DriverCompilerRegistration {
    fn candidate(&self, requirement: &PartitionBackendRequirement) -> Option<BackendCandidate> {
        (self.supports)(requirement).then(|| BackendCandidate {
            name: self.name.to_string(),
            requirement: requirement.clone(),
            priority: self.priority,
        })
    }
}

static WASM_COMPILER: wasm::WasmFamilyCompiler = wasm::WasmFamilyCompiler;
#[cfg(feature = "nyar-vm-lane")]
static NYAR_VM_COMPILER: nyar_vm::NyarVmFamilyCompiler = nyar_vm::NyarVmFamilyCompiler;

#[cfg(feature = "nyar-vm-lane")]
static DRIVER_COMPILERS: [DriverCompilerRegistration; 2] = [
    DriverCompilerRegistration { name: "wasm-binary", priority: 100, supports: wasm::supports_requirement, compiler: &WASM_COMPILER },
    DriverCompilerRegistration { name: "nyar-vm", priority: 100, supports: nyar_vm::supports_requirement, compiler: &NYAR_VM_COMPILER },
];

#[cfg(not(feature = "nyar-vm-lane"))]
static DRIVER_COMPILERS: [DriverCompilerRegistration; 1] =
    [DriverCompilerRegistration { name: "wasm-binary", priority: 100, supports: wasm::supports_requirement, compiler: &WASM_COMPILER }];

/// 返回 bundled driver 编译器注册名（集成测试专用）。
#[doc(hidden)]
pub fn driver_compiler_names() -> Vec<&'static str> {
    DRIVER_COMPILERS.iter().map(|registration| registration.name).collect()
}

pub(crate) fn compile(request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
    let mut selector = BackendSelector::default();
    let mut matched_compilers = Vec::new();
    for registration in DRIVER_COMPILERS.iter() {
        if let Some(candidate) = registration.candidate(&request.requirement) {
            selector.register(candidate);
            matched_compilers.push((registration.name, registration.compiler));
        }
    }

    let Some(selected) = selector.select(&request.requirement)
    else {
        return Err(miette!("`emitter` 找不到满足需求的 backend 编译器：{:?}", request.requirement));
    };
    let Some((_, compiler)) = matched_compilers.into_iter().find(|(name, _)| *name == selected.name)
    else {
        return Err(miette!("选中的 backend `{}` 没有关联 driver compiler", selected.name));
    };
    compiler.compile(request)
}
