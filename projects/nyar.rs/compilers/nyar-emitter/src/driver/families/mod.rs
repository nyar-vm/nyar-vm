use crate::{EmitterError, Result};
use nyar::{BackendCandidate, BackendSelector, PartitionBackendRequirement};

use crate::{DriverCompileReport, DriverCompileRequest};

mod clr;
mod jvm;
#[cfg(feature = "ovm-lane")]
#[doc(hidden)]
pub mod ovm;
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

static CLR_COMPILER: clr::ClrFamilyCompiler = clr::ClrFamilyCompiler;
static JVM_COMPILER: jvm::JvmFamilyCompiler = jvm::JvmFamilyCompiler;
static WASM_COMPILER: wasm::WasmFamilyCompiler = wasm::WasmFamilyCompiler;
#[cfg(feature = "ovm-lane")]
static OVM_COMPILER: ovm::OvmFamilyCompiler = ovm::OvmFamilyCompiler;

#[cfg(feature = "ovm-lane")]
static DRIVER_COMPILERS: [DriverCompilerRegistration; 4] = [
    DriverCompilerRegistration { name: "clr-binary", priority: 100, supports: clr::supports_requirement, compiler: &CLR_COMPILER },
    DriverCompilerRegistration { name: "jvm-jar", priority: 100, supports: jvm::supports_requirement, compiler: &JVM_COMPILER },
    DriverCompilerRegistration { name: "wasm-binary", priority: 100, supports: wasm::supports_requirement, compiler: &WASM_COMPILER },
    DriverCompilerRegistration { name: "ovm", priority: 100, supports: ovm::supports_requirement, compiler: &OVM_COMPILER },
];

#[cfg(not(feature = "ovm-lane"))]
static DRIVER_COMPILERS: [DriverCompilerRegistration; 3] = [
    DriverCompilerRegistration { name: "clr-binary", priority: 100, supports: clr::supports_requirement, compiler: &CLR_COMPILER },
    DriverCompilerRegistration { name: "jvm-jar", priority: 100, supports: jvm::supports_requirement, compiler: &JVM_COMPILER },
    DriverCompilerRegistration { name: "wasm-binary", priority: 100, supports: wasm::supports_requirement, compiler: &WASM_COMPILER },
];

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
        return Err(format!("`emitter` 找不到满足需求的 backend 编译器：{:?}", request.requirement));
    };
    let Some((_, compiler)) = matched_compilers.into_iter().find(|(name, _)| *name == selected.name)
    else {
        return Err(format!("选中的 backend `{}` 没有关联 driver compiler", selected.name));
    };
    compiler.compile(request)
}
