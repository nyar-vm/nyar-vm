//! OVM 库导出 ABI 合同的共享格式定义。
//!
//! 合同是 `.odin` 产物旁的 `<artifact>.ovm-abi.json` sidecar 的唯一 schema。定义放在格式
//! crate 而不是编译器或运行时，是为了让**编译器派生的合同**与**宿主消费的合同**
//! 共用同一份类型：两边都不得各写一份镜像 schema。
//!
//! 失败闭合：
//! - 合同引用的每个布局下标都必须对应 `.odin` 布局表里的真实布局；
//! - 嵌套 Option 必须显式 box，`Some(None)` 与外层 `None` 不得塌陷；
//! - unite 的 tag / payload 槽位是显式事实，消费端不得按对象形状猜；
//! - sidecar 哈希覆盖产物全部字节，对不上即拒绝解释内存。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::OdinLayout;

/// OVM 库导出 ABI schema 版本（合同命名空间 `/v1`）。
pub const OVM_ABI_SCHEMA_VERSION: u32 = 1;

/// OVM 库导出 ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiContract {
    /// schema 版本。
    pub schema_version: u32,
    /// 目标车道标识（固定 `ovm`）。
    pub lane: String,
    /// 按逻辑名排序的导出合同。
    pub exports: Vec<OvmAbiExport>,
    /// 覆盖 `schema_version` 与全部导出的稳定 digest（`sha256-<hex>`）。
    pub digest: String,
}

/// 单个库导出的 OVM ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiExport {
    /// 逻辑导出名。
    pub logical_name: String,
    /// 按声明顺序排列的参数。
    pub params: Vec<OvmAbiParam>,
    /// 返回值结构；无返回值时为 [`OvmAbiType::Unit`]。
    pub returns: OvmAbiType,
    /// 返回值物理载体（来自 plan 冻结合同）。
    pub returns_carrier: String,
}

/// 单个入口参数的 OVM ABI 合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiParam {
    /// 声明下标（0 起连续）。
    pub index: usize,
    /// canonical 参数名。
    pub name: String,
    /// 参数结构。
    #[serde(rename = "type")]
    pub ty: OvmAbiType,
    /// 物理载体（来自 plan 冻结合同）。
    pub carrier: String,
}

/// OVM 物理投影后的 ABI 类型。
///
/// 标量保留语义身份，宿主按该身份编解码；值聚合携带模块布局下标与声明顺序槽位。
/// 引用实例保留不透明身份，宿主不得根据字段形状重建。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmAbiType {
    /// 无值（仅返回位）。
    Unit,
    /// 32-bit 整数。
    I32,
    /// 64-bit 整数。
    I64,
    /// 32-bit 浮点。
    F32,
    /// 64-bit 浮点。
    F64,
    /// 布尔。
    Bool,
    /// Unicode 标量字符。
    Char,
    /// 文本值（`utf8`）。文本不是标量，物理形态由冻结文本边界决定。
    Text {
        /// 文本边界表示。
        representation: OvmTextRepresentation,
    },
    /// 堆数组 `[T]`；宿主按元素结构装配等长槽位对象。
    Array {
        /// 元素结构。
        element: Box<OvmAbiType>,
    },
    /// 定长数组 `[T; N]`。
    FixedArray {
        /// 元素结构。
        element: Box<OvmAbiType>,
        /// 元素数量。
        length: usize,
    },
    /// `T?`：物理表示由 [`OvmOptionBoxing`] 显式声明，宿主不得按“空对象”推断。
    Option {
        /// 非空载荷结构。
        inner: Box<OvmAbiType>,
        /// 显式 box 语义：嵌套 Option 必须带判别 tag，禁止用 `null` 同时表示
        /// 外层 `None` 与内层 `None`。
        boxing: OvmOptionBoxing,
    },
    /// 位置有语义的产品聚合。
    Tuple {
        /// 按声明顺序排列的字段。
        fields: Vec<OvmAbiType>,
    },
    /// 值存储具名聚合：宿主按 `layout` + 槽位装配。
    Aggregate {
        /// 模块布局下标（`.odin` `layouts` 表下标）。
        layout: i32,
        /// 简单名。
        name: String,
        /// 完整命名空间路径。
        module_path: String,
        /// 按声明顺序排列的字段；字段在对象里的槽位等于其下标。
        fields: Vec<OvmAbiField>,
    },
    /// 引用存储名义实例的共享身份（不是字段快照）。
    InstanceHandle {
        /// 模块布局下标。
        layout: i32,
        /// 简单名。
        name: String,
        /// 完整命名空间路径。
        module_path: String,
    },
    /// unite：物理布局由模块 sum 布局下标定位；tag 与 payload 槽位都是 OVM
    /// lowering 已冻结的模块布局事实。
    Unite {
        /// 模块 sum 布局下标（`.odin` `layouts` 表下标）。
        layout: i32,
        /// sum 简单名。
        name: String,
        /// 判别 tag 槽位（OVM 冻结配方恒为 0）。
        tag_slot: usize,
        /// 载荷槽位（OVM 冻结配方恒为 1；零载荷变体在该槽放 `Value::Null`）。
        payload_slot: usize,
        /// 按声明顺序排列的变体。
        variants: Vec<OvmAbiVariant>,
    },
}

impl OvmAbiType {
    /// 返回该类型是否物理上是一个堆句柄。
    pub fn is_handle(&self) -> bool {
        matches!(
            self,
            Self::Array { .. }
                | Self::FixedArray { .. }
                | Self::Option { .. }
                | Self::Tuple { .. }
                | Self::Aggregate { .. }
                | Self::InstanceHandle { .. }
                | Self::Unite { .. }
        )
    }
}

/// 具名聚合字段合同。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiField {
    /// 字段名（来自冻结布局）。
    pub name: String,
    /// 字段结构。
    #[serde(rename = "type")]
    pub ty: OvmAbiType,
}

/// `utf8` 文本值的 OVM 边界表示。
///
/// 判据：文本不是标量。`utf8` 的物理形态由目标计划为**该槽**冻结的文本布局
/// 决定，不由类型名或目标名猜测：
/// - 冻结了文本布局 → 托管文本对象（Reference 存储，单 `[u8]` 槽，UTF-8 字节）；
/// - 没冻结 → raw 宿主串（`Value::String` / JSON string）。
///
/// wasm 不存在这两个分支：JS glue 用 intern 表把字符串映射为 `i32` 句柄，
/// 因此 plan 的 `I32` 只是 wasm 物理事实，OVM 不得复用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmTextRepresentation {
    /// 直接过界：JSON string ↔ `Value::String`。
    Raw,
    /// 托管文本对象：`layout` 的 `byte_slot` 槽是 UTF-8 字节数组。
    Managed {
        /// 文本对象在 `.odin` 布局表里的下标。
        layout: i32,
        /// 文本对象字段总数（冻结配方恒为 1）。
        field_count: usize,
        /// UTF-8 字节数组所在槽位（冻结配方恒为 0）。
        byte_slot: usize,
    },
}

/// unite 变体身份。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiVariant {
    /// 变体名。
    pub name: String,
    /// 判别 tag。
    pub tag: u32,
    /// 变体载荷结构；零载荷变体为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<Box<OvmAbiType>>,
}

/// `Option` 的 OVM 物理 box 语义。
///
/// `Some(None)` 与外层 `None` 在“用 `null` 表示 None”的表示下物理同形——这是
/// 类型信息在边界上的真实丢失，不是序列化细节。因此合同必须显式声明：
///
/// - [`Niche`](Self::Niche)：内层不是 `Option`。`None` = `Value::Null`，
///   `Some(x)` = `x` 本身。
/// - [`Boxed`](Self::Boxed)：内层是 `Option`。三种形态的物理表示分别是
///   `None` = `Value::Null`、
///   `Some(None)` = 2 槽对象 `tag=1, payload=Value::Null`、
///   `Some(Some(x))` = 2 槽对象 `tag=1, payload=x`（`x` 再按内层合同解释）。
///
/// 判别 tag 的冻结配方向宿主暴露为 `tag_slot` / `payload_slot`：`0` 是 `None`，
/// `1` 是 `Some`。消费端只按合同取，禁止“是 null 就当 None”或“空对象当 None”
/// 的推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum OvmOptionBoxing {
    /// 内层物理表示不含 `null` 歧义时使用。
    Niche,
    /// 内层本身是 `Option` 时强制显式 box。
    Boxed {
        /// box 对象在 `.odin` 布局表里的下标（冻结配方恒为 2 槽）。
        layout: i32,
        /// 判别 tag 槽位（冻结配方恒为 0；`0 = None`、`1 = Some`）。
        tag_slot: usize,
        /// 载荷槽位（冻结配方恒为 1）。
        payload_slot: usize,
    },
}

/// OVM 库导出 sidecar 的文件名后缀：每个物理产物各持一份。
pub const OVM_ABI_SIDECAR_SUFFIX: &str = ".ovm-abi.json";

/// 物理产物对应的 sidecar 文件名（`app.odin` → `app.odin.ovm-abi.json`）。
pub fn ovm_abi_sidecar_file(artifact: &str) -> String {
    format!("{artifact}{OVM_ABI_SIDECAR_SUFFIX}")
}

/// OVM 库导出 sidecar（`<artifact>.ovm-abi.json`）：合同 + 它所描述 `.odin` 产物的整体哈希。
///
/// 哈希覆盖产物**全部字节**，不挑字段计算：`.odin` 的布局表、常量池、代码段都
/// 参与 ABI 解释，挑字段迟早漏。宿主读取 sidecar 后必须先按整份产物重算哈希，
/// 对不上即失败闭合，禁止按未校验的布局解释内存。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OvmAbiSidecar {
    /// 导出 ABI 合同。
    pub contract: OvmAbiContract,
    /// 产物文件名（例如 `app.odin`）。
    pub artifact: String,
    /// 产物整体哈希（`sha256-<hex>`）。
    pub artifact_sha256: String,
}

impl OvmAbiSidecar {
    /// 以产物全部字节冻结 sidecar；哈希算法与 [`Self::verify_against`] 唯一。
    pub fn new(contract: OvmAbiContract, artifact: String, artifact_bytes: &[u8]) -> Self {
        Self { contract, artifact, artifact_sha256: artifact_sha256(artifact_bytes) }
    }

    /// 用实际产物字节校验 sidecar；不一致即失败闭合。
    pub fn verify_against(&self, artifact_bytes: &[u8]) -> Result<(), String> {
        let actual = artifact_sha256(artifact_bytes);
        if actual != self.artifact_sha256 {
            return Err(format!(
                "OVM ABI sidecar 与产物 `{}` 哈希不一致：合同 `{}`，实际 `{actual}`；\
                 拒绝按未校验的 ABI 布局解释内存",
                self.artifact, self.artifact_sha256
            ));
        }
        Ok(())
    }
}

/// `.odin` 产物整体 SHA-256（`sha256-<hex>`）。
pub fn artifact_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("sha256-{hex}")
}

impl OvmAbiContract {
    /// 按逻辑名查询导出合同。
    pub fn export(&self, logical_name: &str) -> Option<&OvmAbiExport> {
        self.exports.iter().find(|export| export.logical_name == logical_name)
    }

    /// 校验合同引用的每个布局下标都对应 `.odin` 的真实布局，且物理 recipe 可判。
    ///
    /// 这是“合同与运行时实际布局一致”的机械检查：
    /// - 托管文本的 `layout` / `field_count` / `byte_slot` 必须落在真实布局内；
    /// - 嵌套 Option 必须显式 box（`Niche` 会把 `Some(None)` 与 `None` 塌陷）；
    /// - box 布局必须是 2 槽且 `tag_slot` / `payload_slot` 为冻结配方；
    /// - 聚合 / 序列声明的字段数必须与真实布局一致；
    /// - unite 布局必须是 2 槽且 tag / payload 槽位为冻结配方。
    ///
    /// 任何一项不满足都失败闭合，禁止宿主按另一种解释继续。
    pub fn verify_against_layouts(&self, layouts: &[OdinLayout]) -> Result<(), String> {
        if self.schema_version != OVM_ABI_SCHEMA_VERSION {
            return Err(format!("OVM ABI 合同 schema 版本 {} 与当前支持的 {OVM_ABI_SCHEMA_VERSION} 不一致", self.schema_version));
        }
        for export in &self.exports {
            for param in &export.params {
                verify_type(&param.ty, layouts, &format!("导出 `{}` 参数 {}", export.logical_name, param.name))?;
            }
            verify_type(&export.returns, layouts, &format!("导出 `{}` 返回", export.logical_name))?;
        }
        Ok(())
    }
}

/// 合同结构对 `.odin` 布局表的机械校验（见 [`OvmAbiContract::verify_against_layouts`]）。
fn verify_type(ty: &OvmAbiType, layouts: &[OdinLayout], site: &str) -> Result<(), String> {
    let check_layout = |index: i32, expected: usize, role: &str| -> Result<(), String> {
        let position = usize::try_from(index).map_err(|_| format!("{site} 的 {role} 布局下标 `{index}` 为负"))?;
        let layout =
            layouts.get(position).ok_or_else(|| format!("{site} 的 {role} 布局下标 `{index}` 超出 `.odin` 布局表（{} 项）", layouts.len()))?;
        if usize::try_from(layout.field_count) != Ok(expected) {
            return Err(format!("{site} 的 {role} 声明字段数 {expected} 与 `.odin` 布局 `{index}` 的字段数 {} 不一致", layout.field_count));
        }
        Ok(())
    };
    match ty {
        OvmAbiType::Unit | OvmAbiType::I32 | OvmAbiType::I64 | OvmAbiType::F32 | OvmAbiType::F64 | OvmAbiType::Bool | OvmAbiType::Char => {
            Ok(())
        }
        OvmAbiType::Text { representation: OvmTextRepresentation::Raw } => Ok(()),
        OvmAbiType::Text { representation: OvmTextRepresentation::Managed { layout, field_count, byte_slot } } => {
            check_layout(*layout, *field_count, "托管文本")?;
            if *byte_slot >= *field_count {
                return Err(format!("{site} 的托管文本字节槽 {byte_slot} 超出声明字段数 {field_count}"));
            }
            Ok(())
        }
        OvmAbiType::Array { element } => verify_type(element, layouts, site),
        OvmAbiType::FixedArray { element, .. } => verify_type(element, layouts, site),
        OvmAbiType::Option { inner, boxing } => {
            if matches!(inner.as_ref(), OvmAbiType::Option { .. }) && matches!(boxing, OvmOptionBoxing::Niche) {
                return Err(format!(
                    "{site} 的嵌套 Option 使用 `Niche` 表示：`Some(None)` 与外层 `None` 会塌陷成同一物理值；\
                     嵌套 Option 必须显式 box（tag + payload 槽）"
                ));
            }
            if let OvmOptionBoxing::Boxed { layout, tag_slot, payload_slot } = boxing {
                check_layout(*layout, 2, "Option box")?;
                if *tag_slot != 0 || *payload_slot != 1 {
                    return Err(format!(
                        "{site} 的 Option box 配方必须是 tag_slot=0 / payload_slot=1，实际 tag_slot={tag_slot} / payload_slot={payload_slot}"
                    ));
                }
            }
            verify_type(inner, layouts, site)
        }
        OvmAbiType::Tuple { fields } => {
            for (index, field) in fields.iter().enumerate() {
                verify_type(field, layouts, &format!("{site}::tuple[{index}]"))?;
            }
            Ok(())
        }
        OvmAbiType::Aggregate { layout, fields, name, .. } => {
            check_layout(*layout, fields.len(), &format!("聚合 `{name}`"))?;
            for field in fields {
                verify_type(&field.ty, layouts, &format!("{site}::{}", field.name))?;
            }
            Ok(())
        }
        OvmAbiType::InstanceHandle { layout, name, .. } => {
            let position = usize::try_from(*layout).map_err(|_| format!("{site} 的引用实例 `{name}` 布局下标 `{layout}` 为负"))?;
            layouts.get(position).ok_or_else(|| format!("{site} 的引用实例 `{name}` 布局下标 `{layout}` 超出 `.odin` 布局表"))?;
            Ok(())
        }
        OvmAbiType::Unite { layout, name, tag_slot, payload_slot, variants } => {
            check_layout(*layout, 2, &format!("unite `{name}`"))?;
            if *tag_slot != 0 || *payload_slot != 1 {
                return Err(format!(
                    "{site} 的 unite 配方必须是 tag_slot=0 / payload_slot=1，实际 tag_slot={tag_slot} / payload_slot={payload_slot}"
                ));
            }
            let mut tags = BTreeSet::new();
            for variant in variants {
                if !tags.insert(variant.tag) {
                    return Err(format!("{site} 的 unite `{name}` 变体 `{}` 判别 tag {} 重复", variant.name, variant.tag));
                }
                if let Some(payload) = &variant.payload {
                    verify_type(payload, layouts, &format!("{site}::{}", variant.name))?;
                }
            }
            Ok(())
        }
    }
}
