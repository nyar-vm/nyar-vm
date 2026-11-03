//! 字体栈与打包策略（草案）。
//!
//! 打包策略：阶段一不捆绑字体文件，按脚本声明系统字体栈（零产物成本、零许可
//! 风险）；度量由布局层自持，字体栈只是度量后端的候选顺序。后续阶段若需要
//! 跨宿主一致性，改为子集化捆绑（按 Unicode range 切子集、manifest 登记
//! digest），合同不变、只换度量后端实现。
//!
//! 跨平台差异点（冻结进合同注释，防后续阶段还债）：
//! 1. **度量差异**：CJK 表意文字在各平台字体里都是全角 1.0em，度量稳定；拉丁
//!    字面宽度随字体家族漂移（±8% 量级），因此 `Content` 尺寸约束在跨平台只
//!    保证行盒数与断行位置在相同度量后端下一致，不保证像素级一致。
//! 2. **fallback 可用性**：macOS 有 PingFang SC、Windows 有 Microsoft YaHei、
//!    Linux/Android 依赖 Noto Sans CJK；栈序必须把「必有兜底」放在最后
//!    （sans-serif / system-ui），缺字时度量后端回退到桩度量并标记近似。
//! 3. **hinting 与行高**：Windows DirectWrite 与 macOS CoreText 的 hinting 策略
//!    不同，行高取合同值（token 的 line-height 倍率）而非宿主默认，避免行盒漂移。

/// 脚本类别：fallback 顺序按脚本声明，不按语言区域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Script {
    /// 拉丁及通用西文脚本。
    Latin,
    /// 中日韩表意文字脚本。
    Cjk,
}

/// 运行平台（度量后端与字体栈的寻址维度）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS（CoreText）。
    MacOs,
    /// Windows（DirectWrite）。
    Windows,
    /// Linux（fontconfig）。
    Linux,
    /// Android。
    Android,
    /// 浏览器（dom/html 与 webview 阶段）。
    Web,
}

/// 某脚本在某平台的 fallback 顺序（首选项优先，末位是必有兜底）。
pub fn fallback_stack(script: Script, platform: Platform) -> Vec<&'static str> {
    match (script, platform) {
        (Script::Latin, Platform::MacOs) => vec!["system-ui", "Helvetica Neue", "sans-serif"],
        (Script::Latin, Platform::Windows) => vec!["system-ui", "Segoe UI", "sans-serif"],
        (Script::Latin, _) => vec!["system-ui", "Roboto", "sans-serif"],
        (Script::Cjk, Platform::MacOs) => vec!["PingFang SC", "Hiragino Sans GB", "Noto Sans CJK SC", "sans-serif"],
        (Script::Cjk, Platform::Windows) => vec!["Microsoft YaHei", "Noto Sans CJK SC", "sans-serif"],
        (Script::Cjk, Platform::Linux) => vec!["Noto Sans CJK SC", "Source Han Sans SC", "sans-serif"],
        (Script::Cjk, Platform::Android) => vec!["Noto Sans CJK SC", "sans-serif"],
        (Script::Cjk, Platform::Web) => vec!["PingFang SC", "Microsoft YaHei", "Noto Sans CJK SC", "sans-serif"],
    }
}

/// 字体打包策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontPackaging {
    /// 阶段一：只用系统栈，不捆绑字体文件。
    SystemStackOnly,
    /// 后续：按 Unicode range 子集化捆绑，manifest 登记 digest。
    SubsetBundle {
        /// 子集 digest 是否已登记进 manifest。
        digest_registered: bool,
    },
}

/// 阶段一策略。
pub const STAGE1_PACKAGING: FontPackaging = FontPackaging::SystemStackOnly;
