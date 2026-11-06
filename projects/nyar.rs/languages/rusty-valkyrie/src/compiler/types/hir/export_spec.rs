//! `[export(..)]` 注解在 HIR 项上的元数据。

use super::{HirArgument, HirAttribute, HirExpr, HirExprKind, HirLiteral, HirStringSegment};
use crate::compiler::types::{Identifier, NamePath};
use nyar_types::{AttributeId, AttributeRegistry, LibraryGlueOperation, builtin_attribute};
use std::sync::{Mutex, OnceLock};

/// `[export(case: ...)]` 合同允许的取值集合（唯一事实来源）。
pub const EXPORT_CASES: [&str; 2] = ["camelCase", "snake_case"];

/// CLR / wasm / Nyar 模块表面上的导出分区与可选重命名。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirExportSpec {
    /// 目标导出分区。空表示使用点上的 `default`。
    pub partitions: Vec<String>,
    /// 可选的导出名覆盖（`name: "twoSum"`）。
    pub export_name: Option<String>,
    /// 可选的大小写变换（`case: "camelCase"`）。
    pub export_case: Option<String>,
}

impl HirExportSpec {
    /// 产物路由用的主分区。
    pub fn primary_partition(&self) -> String {
        self.partitions.first().cloned().unwrap_or_else(|| "default".to_string())
    }

    /// 由本地 `micro` 名解析 wasm / 宿主导出符号。
    ///
    /// 非法 `case` 不得被当成导出名；未知 case 回退为本地名（诊断由调用方负责）。
    pub fn resolve_exported_name(&self, local_name: &Identifier) -> String {
        if let Some(name) = &self.export_name {
            return name.clone();
        }
        match self.export_case.as_deref() {
            Some("camelCase") => snake_case_to_camel_case(local_name.as_str()),
            Some("snake_case") | None => local_name.as_str().to_string(),
            Some(_) => local_name.as_str().to_string(),
        }
    }
}

/// 解析阶段共享的属性注册表（内建播种 + 用户属性 intern）。
///
/// 完整编译会话上下文接入前，用进程内单例保证同名 → 同 id；重复键失败关闭由
/// [`AttributeRegistry::intern_unique`] 提供，此处 `intern` 对同名幂等。
fn attribute_registry() -> &'static Mutex<AttributeRegistry> {
    static REGISTRY: OnceLock<Mutex<AttributeRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(AttributeRegistry::with_builtins()))
}

/// 将属性简单名解析为 [`AttributeId`]（解析边界；之后只比较 id）。
///
/// 内建属性走播种槽；用户属性由 [`AttributeRegistry::intern`] 分配。
pub fn resolve_attribute_id(attribute: &HirAttribute) -> Option<AttributeId> {
    let name = attribute.name.parts().last()?.as_str();
    let Ok(mut registry) = attribute_registry().lock()
    else {
        return None;
    };
    Some(registry.intern(name))
}

/// 解析 `[workload_phase("request")]` / `[workload_phase(name: "request")]` 的阶段名。
///
/// 空名或缺失字符串字面量时返回 `None`（失败闭合，不发明默认阶段）。
pub fn parse_workload_phase_from_annotations(annotations: &[HirAttribute]) -> Option<String> {
    let attribute = annotations.iter().find(|attribute| resolve_attribute_id(attribute) == Some(builtin_attribute::workload_phase()))?;

    if attribute.arguments.is_empty() {
        return None;
    }

    for argument in &attribute.arguments {
        if let Some(key) = argument.key.as_ref() {
            if key.as_str() == "name" {
                let name = argument_string_literal(argument)?;
                return non_empty_phase_name(name);
            }
            continue;
        }
        if let Some(name) = argument_string_literal(argument) {
            return non_empty_phase_name(name);
        }
    }
    None
}

fn non_empty_phase_name(name: String) -> Option<String> {
    if name.is_empty() { None } else { Some(name) }
}

/// 解析 `[library_glue(construct)]` / `[library_glue(role: "push")]` 的宿主直通角色。
///
/// 角色只由语言声明给出，这是「哪个成员实现哪种宿主直通操作」的唯一事实来源；
/// 调用方不得按成员名猜映射。未知角色或缺失参数时返回 `None`（失败闭合）。
pub fn parse_library_glue_operation(annotations: &[HirAttribute]) -> Option<LibraryGlueOperation> {
    let attribute = annotations
        .iter()
        .find(|attribute| attribute.name.parts().last().map(|part| part.as_str()) == Some(LibraryGlueOperation::ATTRIBUTE_NAME))?;

    for argument in &attribute.arguments {
        if let Some(key) = argument.key.as_ref() {
            if matches!(key.as_str(), "role" | "operation") {
                return LibraryGlueOperation::from_role_name(&annotation_argument_name(argument)?);
            }
            continue;
        }
        return LibraryGlueOperation::from_role_name(&annotation_argument_name(argument)?);
    }
    None
}

/// 从标注参数取角色名：接受字符串字面量与裸路径（`construct` / `at`）。
fn annotation_argument_name(argument: &HirArgument) -> Option<String> {
    if let Some(name) = argument_string_literal(argument) {
        return Some(name);
    }
    let HirExprKind::Path(path) = &argument.value.kind
    else {
        return None;
    };
    path.parts().last().map(|part| part.as_str().to_string())
}

/// 解析 `[export]` / `[export(unity.runtime)]` / `[export(name: "twoSum")]` / `[export(case: "camelCase")]`。
pub fn parse_export_spec_from_annotations(annotations: &[HirAttribute]) -> Option<HirExportSpec> {
    let attribute = annotations.iter().find(|attribute| resolve_attribute_id(attribute) == Some(builtin_attribute::export()))?;

    if attribute.arguments.is_empty() {
        return Some(HirExportSpec { partitions: vec!["default".to_string()], export_name: None, export_case: None });
    }

    let mut partitions = Vec::new();
    let mut export_name = None;
    let mut export_case = None;

    for argument in &attribute.arguments {
        if let Some(key) = argument.key.as_ref() {
            let key = key.as_str();
            if key == "name" {
                export_name = argument_string_literal(argument);
            }
            else if key == "case" {
                let Some(value) = argument_string_literal(argument)
                else {
                    return None;
                };
                if EXPORT_CASES.contains(&value.as_str()) {
                    export_case = Some(value);
                }
                else {
                    // 非法 case：整条 export 合同失败闭合，禁止把拼写当导出名；
                    // 注解处的结构化诊断由 `invalid_export_case_argument` 提供。
                    return None;
                }
            }
            else {
                export_name.get_or_insert_with(|| key.to_string());
            }
            continue;
        }

        if let Some(partition) = export_arg_to_partition(&argument.value) {
            partitions.push(partition);
        }
    }

    if partitions.is_empty() {
        partitions.push("default".to_string());
    }

    Some(HirExportSpec { partitions, export_name, export_case })
}

/// 提取 `[export(case: ...)]` 中不在合同枚举内的取值及其实参表达式。
///
/// 返回 `None` 表示没有 `[export]` 注解或 `case` 实参合法。字符串字面量逐字返回，
/// 非字符串实参记为 `<non-string literal>`；调用方在注解处结构化失败，禁止因为
/// `parse_export_spec_from_annotations` 静默返回 `None` 而退化成下游缺少导出错误。
pub fn invalid_export_case_argument(annotations: &[HirAttribute]) -> Option<(String, &HirExpr)> {
    let attribute = annotations.iter().find(|attribute| resolve_attribute_id(attribute) == Some(builtin_attribute::export()))?;

    for argument in &attribute.arguments {
        if argument.key.as_ref().map(|key| key.as_str()) != Some("case") {
            continue;
        }
        let value = argument_string_literal(argument).unwrap_or_else(|| "<non-string literal>".to_string());
        if !EXPORT_CASES.contains(&value.as_str()) {
            return Some((value, &argument.value));
        }
    }
    None
}

fn argument_string_literal(argument: &HirArgument) -> Option<String> {
    let HirExprKind::Literal(HirLiteral::String(literal)) = &argument.value.kind
    else {
        return None;
    };

    let mut rendered = String::new();
    for segment in &literal.segments {
        let HirStringSegment::Text(text) = segment
        else {
            return None;
        };
        rendered.push_str(text);
    }
    Some(rendered)
}

fn export_arg_to_partition(expr: &HirExpr) -> Option<String> {
    match &expr.kind {
        HirExprKind::Literal(HirLiteral::String(literal)) => {
            let mut rendered = String::new();
            for segment in &literal.segments {
                let HirStringSegment::Text(text) = segment
                else {
                    return None;
                };
                rendered.push_str(text);
            }
            Some(rendered)
        }
        HirExprKind::Path(path) => Some(path_to_partition(path)),
        _ => None,
    }
}

fn path_to_partition(path: &NamePath) -> String {
    path.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join(".")
}

/// `two_sum` → `twoSum`（`export(case: "camelCase")` 命名变换）。
///
/// 按 `_` 分段处理，规则以段为单位而不是逐字符拼装：
/// - 首段原样保留（含已驼峰输入，无下划线时原样返回）；
/// - 后续段首字母大写，其余字符原样；
/// - 连续的单字母段视为一个词，只大写该 run 的首字母，其余字母不强制大写
///   （`o_r_s` → `ors`，而不是 `ORS`）；数字段没有大小写，原样输出并切断连读；
/// - 前导/尾随/连续下划线不产生字符，但会切断单字母连读，后续段仍算新词，
///   不吞掉词边界（`a_b__c` → `aBC`）。
///
/// `subarray_bitwise_o_r_s` → `subarrayBitwiseOrs`。
pub fn snake_case_to_camel_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut first_segment_seen = false;
    let mut previous_single_letter = false;
    for segment in name.split('_') {
        if segment.is_empty() {
            previous_single_letter = false;
            continue;
        }
        if !first_segment_seen {
            out.push_str(segment);
            first_segment_seen = true;
            previous_single_letter = false;
            continue;
        }
        let mut chars = segment.chars();
        let first = chars.next().expect("non-empty segment");
        if previous_single_letter && is_single_letter_segment(segment) {
            out.push(first);
        }
        else {
            out.extend(first.to_uppercase());
        }
        out.extend(chars);
        previous_single_letter = is_single_letter_segment(segment);
    }
    out
}

fn is_single_letter_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    matches!((chars.next(), chars.next()), (Some(ch), None) if ch.is_alphabetic())
}
