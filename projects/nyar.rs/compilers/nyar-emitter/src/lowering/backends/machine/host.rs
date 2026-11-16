//! 原生宿主身份与产物目标（target-lane 私有服务）。
//!
//! 平台身份（架构 + OS 规格 + ABI → 容器 / 调用约定）与宿主模块类型（Node-API /
//! CPython 的入口符号与导入面）都是**目标侧事实**：语言适配器只提交已冻结的
//! [`nyar::target_resolution::ResolvedTarget`] 与导出身份，不各自再猜一次平台或
//! 宿主边界。本模块据此把「普通共享库」与「宿主扩展」分流到对应的承载合同。

use nyar::abstractions::{CanonicalAbi, CanonicalArch, CanonicalSpecification, CanonicalTarget};
use nyar::target_resolution::{ArtifactContract, CpythonHostConfig, ResolvedTarget};

use super::X64CallConv;

/// 容器写入器：由平台身份的 specification 决定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerKind {
    /// `PE32+`。
    Pe,
    /// `ELF64 ET_DYN`。
    Elf,
}

/// 已冻结的原生平台身份。
///
/// 架构、OS 规格与 ABI 一起决定容器和调用约定，而不是由「OS 枚举」再反推 ABI。
/// 只接受当前文本 LIR 真正闭合的组合；其余平台在构造时明确失败，不降级成最近的
/// 可用平台。完整身份（[`Self::canonical`]）保留给缓存与产物记录。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X64Platform {
    target: CanonicalTarget,
    /// 文本 LIR 实际消费的整数调用约定。
    call_conv: X64CallConv,
    container: ContainerKind,
}

impl X64Platform {
    /// 从已解析的平台身份构造；未实现的能力在这里结构化失败。
    pub fn from_canonical(target: CanonicalTarget) -> Result<Self, String> {
        let abi = target.abi.unwrap_or_else(|| target.resolved_abi());
        let target = CanonicalTarget { abi: Some(abi), ..target };
        let (call_conv, container) = match (target.arch, target.specification, abi) {
            // x86-64 Windows 的 MSVC 与 GNU 目标共用 Win64 与 PE 容器。
            (CanonicalArch::X86_64, CanonicalSpecification::Windows, CanonicalAbi::Msvc | CanonicalAbi::Gnu) => {
                (X64CallConv::Win64, ContainerKind::Pe)
            }
            (CanonicalArch::X86_64, CanonicalSpecification::Linux, CanonicalAbi::Gnu | CanonicalAbi::SystemV) => {
                (X64CallConv::SystemV64, ContainerKind::Elf)
            }
            (arch, specification, abi) => {
                return Err(format!(
                    "原生残差产品尚不支持平台身份 `{}-unknown-{}-{}`：文本 LIR 只闭合 x86-64 的 Win64 / SysV64 调用约定",
                    arch.as_str(),
                    specification.as_str(),
                    abi.as_str()
                ));
            }
        };
        Ok(Self { target, call_conv, container })
    }

    /// x86-64 Windows 平台（`x86_64-pc-windows-msvc`）。
    pub fn windows_x64() -> Self {
        Self::from_canonical(CanonicalTarget::parse("x86_64-pc-windows-msvc").expect("canonical windows x64 target")).expect("windows x64 is supported")
    }

    /// x86-64 Linux 平台（`x86_64-unknown-linux-gnu`）。
    pub fn linux_x64() -> Self {
        Self::from_canonical(CanonicalTarget::parse("x86_64-unknown-linux-gnu").expect("canonical linux x64 target")).expect("linux x64 is supported")
    }

    /// 完整平台身份（含冻结 ABI），用于缓存与产物记录。
    pub fn canonical(self) -> CanonicalTarget {
        self.target
    }

    /// 文本 LIR 的整数调用约定。
    pub fn abi(self) -> X64CallConv {
        self.call_conv
    }

    /// 是否为 `PE32+` / Win64 平台。
    pub fn is_windows(self) -> bool {
        self.container == ContainerKind::Pe
    }

    /// 平台名称，用于结构化错误。
    pub fn name(self) -> String {
        self.target.to_string()
    }
}

/// 宿主扩展模块类型：决定入口符号、导入面与宿主边界包装。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostModule {
    /// Node-API：`napi_register_module_v1` + `napi_*`。
    Node,
    /// CPython：`PyInit_<module>` + `PyModule_*` / `PyArg_ParseTuple`。
    Python,
}

impl HostModule {
    /// 模块类型名称，用于结构化错误。
    pub fn name(self) -> &'static str {
        match self {
            Self::Node => "Node-API",
            Self::Python => "CPython",
        }
    }
}

/// 原生产物目标：已冻结的平台身份与宿主模块类型正交，不再用「Node/Python 等于 Linux」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum X64ProductTarget {
    /// 普通共享库：宿主按导出函数 ABI 直接调用（`dlopen` / `LoadLibrary`）。
    Library(X64Platform),
    /// 宿主扩展模块：`platform` 决定容器与调用约定，`module` 决定入口与导入面，
    /// 探测/显式配置冻结的 CPython ABI（扩展后缀、导入库）随合同一起进入发射。
    Extension {
        /// 已冻结的平台身份。
        platform: X64Platform,
        /// 宿主模块类型。
        module: HostModule,
        /// CPython 宿主 ABI 配置（Node 为 `None`）：Windows 导入 DLL 名与扩展后缀
        /// 都是版本相关的宿主事实，由解析服务一次冻结，lane 不再按 OS 猜。
        cpython: Option<CpythonHostConfig>,
    },
}

impl X64ProductTarget {
    /// 从统一解析服务的结果构造产物目标。
    pub fn from_resolved(resolved: &ResolvedTarget) -> Result<Self, String> {
        Self::from_platform_and_contract(resolved.platform, &resolved.contract)
    }

    /// 从平台身份与产物合同构造产物目标（正式 lane 适配器路径）。
    ///
    /// 平台能力不足（非 x86-64 / 未支持 OS）与平台 lane 合同都在这里结构化失败。
    pub fn from_platform_and_contract(platform: CanonicalTarget, contract: &ArtifactContract) -> Result<Self, String> {
        let platform = X64Platform::from_canonical(platform)?;
        match contract {
            ArtifactContract::Native => Ok(Self::Library(platform)),
            ArtifactContract::NodeApi => Ok(Self::Extension { platform, module: HostModule::Node, cpython: None }),
            ArtifactContract::Cpython(config) => {
                Ok(Self::Extension { platform, module: HostModule::Python, cpython: Some(config.clone()) })
            }
            ArtifactContract::Platform => Err("平台 lane 合同不是原生宿主产物".to_string()),
        }
    }

    /// 便捷构造：无宿主 ABI 配置的扩展目标（残差与测试路径）。
    pub fn extension(platform: X64Platform, module: HostModule) -> Self {
        Self::Extension { platform, module, cpython: None }
    }

    /// 探测/配置冻结的 CPython 导入 DLL 名。
    pub fn cpython_import_library(&self) -> Option<&str> {
        match self {
            Self::Extension { cpython: Some(config), .. } => config.import_library.as_deref(),
            _ => None,
        }
    }

    /// 探测/配置冻结的 CPython 扩展后缀（如 `.cp312-win_amd64.pyd`）；缺失时由
    /// 调用方按平台规范名回落，并在产物身份中体现配置差异。
    pub fn cpython_module_suffix(&self) -> Option<&str> {
        match self {
            Self::Extension { cpython: Some(config), .. } => config.module_suffix.as_deref(),
            _ => None,
        }
    }

    /// 产物平台身份。
    pub fn platform(&self) -> X64Platform {
        match self {
            Self::Library(platform) | Self::Extension { platform, .. } => *platform,
        }
    }

    /// 宿主模块类型（普通库为 `None`）。
    pub fn host_module(&self) -> Option<HostModule> {
        match self {
            Self::Library(_) => None,
            Self::Extension { module, .. } => Some(*module),
        }
    }

    /// 对应的文本 LIR 调用约定。
    pub fn abi(&self) -> X64CallConv {
        self.platform().abi()
    }

    /// effect 通道是否由宿主作为隐藏首个整数形参传入。
    ///
    /// 普通库由宿主传句柄/fd；扩展模块自己写 stdout。
    pub fn sink_from_parameter(&self) -> bool {
        self.host_module().is_none()
    }

    /// 是否为 Windows 平台。
    pub fn is_windows(&self) -> bool {
        self.platform().is_windows()
    }

    /// 目标名称，用于结构化错误。
    pub fn name(&self) -> String {
        match self {
            Self::Library(platform) => format!("{} shared library", platform.name()),
            Self::Extension { platform, module, .. } => format!("{} {} extension", platform.name(), module.name()),
        }
    }

    /// 扩展方法允许的整数形参上限。
    ///
    /// 由宿主的固定实参占用与目标 ABI 的寄存器形参共同决定。
    pub fn method_parameter_limit(&self) -> usize {
        match self {
            Self::Library(platform) => platform.abi().integer_argument_limit(),
            Self::Extension { platform, module: HostModule::Node, .. } => platform.abi().integer_argument_limit(),
            // `PyArg_ParseTuple(args, fmt, &a…)` 占两个固定实参。
            Self::Extension { platform, module: HostModule::Python, .. } => {
                platform.abi().integer_argument_limit().saturating_sub(2)
            }
        }
    }
}
