//! 作者面目标请求的统一解析服务。
//!
//! 作者写的 `native` / `node` / `python` 是**占位符**，不是平台身份。本模块在建立
//! 编译计划时把它们展开为三个相互独立的事实，并在布局、ABI、LIR、链接与缓存身份
//! 冻结之前完成：
//!
//! - 原始请求（[`TargetRequest`]）；
//! - 具体机器平台身份（[`CanonicalTarget`]，arch / vendor / specification / ABI）；
//! - 产物与宿主合同（[`ArtifactContract`]）。
//!
//! 解析只在这里发生。parser、Legion、LegendVM、emitter 与打包器都必须消费同一份
//! 冻结结果；无法识别的架构或 OS 明确失败，不得回退到「最常见的平台」。生成后的
//! 机器码不再临时决定自己采用哪种 ABI。

use std::fmt::{Display, Formatter};

use crate::abstractions::{CanonicalAbi, CanonicalArch, CanonicalSpecification, CanonicalTarget, CanonicalTargetParseError, CanonicalVendor};

/// 目标解析错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetResolutionError {
    message: String,
}

impl TargetResolutionError {
    /// 构造解析错误。
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// 错误文本。
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Display for TargetResolutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.message, f)
    }
}

impl std::error::Error for TargetResolutionError {}

impl From<CanonicalTargetParseError> for TargetResolutionError {
    fn from(error: CanonicalTargetParseError) -> Self {
        Self::new(error.to_string())
    }
}

/// 作者面目标请求。
///
/// 别名是占位符，必须由 [`TargetResolver`] 展开；`Platform` 是作者直接写出的三元组
/// 或四元组，本身已经是平台身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRequest {
    /// `native`：宿主平台 + 普通原生产物（可执行文件或共享库由构建声明选择）。
    Native,
    /// `node`：宿主平台 + Node-API 扩展合同。
    Node,
    /// `python`：宿主平台 + CPython 扩展合同。
    Python,
    /// 显式平台身份（三元组 / 四元组）。
    Platform(CanonicalTarget),
}

impl TargetRequest {
    /// 解析作者面目标请求。
    ///
    /// `node` / `python` 在这里被识别为宿主合同占位符，不会再被
    /// [`CanonicalTarget::parse`] 解释成 Wasm 目标。
    pub fn parse(value: &str) -> Result<Self, TargetResolutionError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(TargetResolutionError::new("目标请求不能为空"));
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "native" => Ok(Self::Native),
            "node" => Ok(Self::Node),
            "python" => Ok(Self::Python),
            _ => CanonicalTarget::parse(trimmed).map(Self::Platform).map_err(TargetResolutionError::from),
        }
    }

    /// 是否为需要展开的占位符。
    pub fn is_placeholder(self) -> bool {
        !matches!(self, Self::Platform(_))
    }
}

impl Display for TargetRequest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native => f.write_str("native"),
            Self::Node => f.write_str("node"),
            Self::Python => f.write_str("python"),
            Self::Platform(target) => Display::fmt(target, f),
        }
    }
}

/// CPython 扩展的宿主配置。
///
/// 扩展后缀与导入库取决于将要装载产物的解释器，不能只凭 OS 拼出来；缺失时由需要
/// 它们的阶段明确失败，而不是猜一个默认值。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CpythonHostConfig {
    /// 扩展模块后缀，例如 `.pyd` / `.so`。
    pub module_suffix: Option<String>,
    /// Windows 上 CPython 扩展链接的导入库名（含版本）。
    pub import_library: Option<String>,
}

/// 产物与宿主合同。
///
/// 它只描述产物形态与宿主边界，不重复平台身份；二者必须同时保留。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactContract {
    /// 平台 lane 自身产物（Wasm / CLR / JVM / OVM），没有原生宿主包装。
    Platform,
    /// 普通原生产物：可执行文件或共享库，由构建声明选择。
    Native,
    /// Node-API 扩展（`.node`）。
    NodeApi,
    /// CPython 扩展（`.pyd` / `.so`）。
    Cpython(CpythonHostConfig),
}

impl ArtifactContract {
    /// 合同标签，用于缓存与产物目录身份。
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Native => "native",
            Self::NodeApi => "node-api",
            Self::Cpython(_) => "cpython",
        }
    }
}

/// 解析后冻结的目标：请求、平台身份与宿主合同同时保留。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedTarget {
    /// 作者面原始请求。
    pub request: TargetRequest,
    /// 具体机器平台身份；ABI 已经冻结，不再由后续阶段临时推导。
    pub platform: CanonicalTarget,
    /// 产物与宿主合同。
    pub contract: ArtifactContract,
}

impl ResolvedTarget {
    /// 缓存与产物目录使用的完整身份。
    ///
    /// 同一平台上的普通原生产物与 Node-API / CPython 扩展必须可以区分。
    pub fn identity(&self) -> String {
        match &self.contract {
            ArtifactContract::Platform | ArtifactContract::Native => self.platform.to_string(),
            ArtifactContract::NodeApi => format!("{}-node-api", self.platform),
            ArtifactContract::Cpython(_) => format!("{}-cpython", self.platform),
        }
    }
}

/// 目标解析器：持有宿主平台，把作者面请求展开为冻结目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetResolver {
    host: CanonicalTarget,
}

impl TargetResolver {
    /// 以本机环境为宿主创建解析器。
    ///
    /// 未知架构或 OS 明确失败，不做平台猜测。
    pub fn for_build_host() -> Result<Self, TargetResolutionError> {
        Ok(Self { host: probe_build_host()? })
    }

    /// 用显式宿主平台创建解析器（交叉编译时覆盖本机探测）。
    pub fn with_host(host: CanonicalTarget) -> Self {
        Self { host: freeze(host) }
    }

    /// 当前宿主平台。
    pub fn host(&self) -> CanonicalTarget {
        self.host
    }

    /// 解析目标请求；占位符使用当前宿主平台展开。
    pub fn resolve(&self, request: TargetRequest) -> Result<ResolvedTarget, TargetResolutionError> {
        match request {
            TargetRequest::Platform(platform) => self.resolve_explicit(request, platform),
            placeholder => self.resolve_placeholder(placeholder, self.host),
        }
    }

    /// 用显式平台覆盖宿主展开占位符（跨平台构建）。
    pub fn resolve_with_platform(&self, request: TargetRequest, platform: CanonicalTarget) -> Result<ResolvedTarget, TargetResolutionError> {
        match request {
            TargetRequest::Platform(_) => self.resolve_explicit(request, platform),
            placeholder => self.resolve_placeholder(placeholder, platform),
        }
    }

    /// 解析目标标签（别名、三元组或四元组）。
    pub fn resolve_label(&self, value: &str) -> Result<ResolvedTarget, TargetResolutionError> {
        self.resolve(TargetRequest::parse(value)?)
    }

    fn resolve_explicit(&self, request: TargetRequest, platform: CanonicalTarget) -> Result<ResolvedTarget, TargetResolutionError> {
        let platform = freeze(platform);
        let contract = match platform.arch {
            CanonicalArch::Native => {
                return Err(TargetResolutionError::new(format!(
                    "平台身份 `{platform}` 含未展开的 `native` 架构；`native` 是请求占位符，请用 `TargetRequest` 解析"
                )));
            }
            CanonicalArch::Clr | CanonicalArch::Jvm | CanonicalArch::Wasm32 | CanonicalArch::Wasm64 | CanonicalArch::Ovm => {
                ArtifactContract::Platform
            }
            _ => ArtifactContract::Native,
        };
        Ok(ResolvedTarget { request, platform, contract })
    }

    fn resolve_placeholder(&self, request: TargetRequest, platform: CanonicalTarget) -> Result<ResolvedTarget, TargetResolutionError> {
        let platform = require_machine_platform(request, platform)?;
        let contract = match request {
            TargetRequest::Native => ArtifactContract::Native,
            TargetRequest::Node => ArtifactContract::NodeApi,
            TargetRequest::Python => ArtifactContract::Cpython(CpythonHostConfig::default()),
            TargetRequest::Platform(_) => unreachable!("显式平台不走占位符展开"),
        };
        Ok(ResolvedTarget { request, platform, contract })
    }
}

/// 探测构建宿主的机器平台。
///
/// 只接受可以确切识别的架构与 OS；未知值直接失败。ABI 由平台身份本身给出，
/// 不是「Windows 就叫 MSVC、其余都叫 GNU」式猜测。
pub fn probe_build_host() -> Result<CanonicalTarget, TargetResolutionError> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => CanonicalArch::X86_64,
        "aarch64" => CanonicalArch::AArch64,
        "x86" => CanonicalArch::X86,
        "arm" => CanonicalArch::Arm,
        "riscv64" => CanonicalArch::RiscV64,
        "riscv32" => CanonicalArch::RiscV32,
        other => {
            return Err(TargetResolutionError::new(format!(
                "无法识别构建宿主的 CPU 架构 `{other}`；请显式指定目标三元组或四元组"
            )));
        }
    };
    let (vendor, specification, abi) = match std::env::consts::OS {
        "windows" => (CanonicalVendor::Pc, CanonicalSpecification::Windows, CanonicalAbi::Msvc),
        "macos" => (
            CanonicalVendor::Apple,
            CanonicalSpecification::MacOs,
            if arch == CanonicalArch::AArch64 { CanonicalAbi::Aapcs64 } else { CanonicalAbi::SystemV },
        ),
        "linux" => (CanonicalVendor::Unknown, CanonicalSpecification::Linux, CanonicalAbi::Gnu),
        other => {
            return Err(TargetResolutionError::new(format!(
                "无法识别构建宿主的操作系统 `{other}`；请显式指定目标三元组或四元组"
            )));
        }
    };
    Ok(CanonicalTarget::new(arch, vendor, specification, Some(abi)))
}

/// 冻结平台身份的 ABI：缺失 ABI 的三元组按平台身份派生一次，之后不再重算。
fn freeze(target: CanonicalTarget) -> CanonicalTarget {
    CanonicalTarget { abi: Some(target.abi.unwrap_or_else(|| target.resolved_abi())), ..target }
}

/// 占位符只能展开到真实的机器平台。
fn require_machine_platform(request: TargetRequest, platform: CanonicalTarget) -> Result<CanonicalTarget, TargetResolutionError> {
    let platform = freeze(platform);
    match platform.arch {
        CanonicalArch::X86 | CanonicalArch::X86_64 | CanonicalArch::AArch64 | CanonicalArch::Arm | CanonicalArch::RiscV32 | CanonicalArch::RiscV64 => {}
        other => {
            return Err(TargetResolutionError::new(format!(
                "`{request}` 是原生宿主占位符，但平台 `{platform}` 的架构是 `{}`，不是机器架构",
                other.as_str()
            )));
        }
    }
    match platform.specification {
        CanonicalSpecification::Windows
        | CanonicalSpecification::Linux
        | CanonicalSpecification::MacOs
        | CanonicalSpecification::Android
        | CanonicalSpecification::Ios => {}
        other => {
            return Err(TargetResolutionError::new(format!(
                "`{request}` 需要已冻结的操作系统平台，而 `{platform}` 的规格是 `{}`",
                other.as_str()
            )));
        }
    }
    Ok(platform)
}
