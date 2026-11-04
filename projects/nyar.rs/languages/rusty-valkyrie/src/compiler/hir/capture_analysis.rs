//! `HIR` 闭包捕获分析的最小前端数据结构。
use std::collections::BTreeMap;

use crate::compiler::types::{
    Identifier, SourceID, SourceSpan,
    hir::{CaptureMode, CaptureStorage, HirCapture, HirIdentifier, ValkyrieType},
};

/// 跟踪外层作用域中被闭包捕获的值。
#[derive(Debug, Default)]
pub struct CaptureAnalyzer {
    bindings: BTreeMap<String, CaptureBinding>,
    captured: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CaptureBinding {
    ty: ValkyrieType,
    is_mutable: bool,
}

impl CaptureAnalyzer {
    /// 创建空的捕获分析器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册当前可见变量。
    pub fn add_var(&mut self, name: &str, ty: ValkyrieType, is_mutable: bool) {
        self.bindings.insert(name.to_string(), CaptureBinding { ty, is_mutable });
    }

    /// 标记当前可见变量已被闭包捕获。
    ///
    /// `is_write` 记录 lambda 体内对该绑定是否有写入；P3 的可变捕获 cell 需要
    /// 该事实，任何调用方都不得丢弃。
    pub fn access_var(&mut self, name: &str, is_write: bool) {
        if self.bindings.contains_key(name) {
            let entry = self.captured.entry(name.to_string()).or_insert(false);
            *entry = *entry || is_write;
        }
    }

    /// 导出 `HIR` 捕获列表。
    pub fn into_captures(self) -> Vec<HirCapture> {
        self.captured
            .into_iter()
            .filter_map(|(name, is_write)| {
                let binding = self.bindings.get(&name)?;
                Some(HirCapture {
                    identifier: HirIdentifier {
                        name: Identifier::new(&name),
                        shadow_index: 0,
                        span: SourceSpan::new(SourceID::default(), 0, 0),
                    },
                    ty: binding.ty.clone(),
                    mode: capture_mode(&binding.ty),
                    is_mutable: binding.is_mutable,
                    is_write,
                    storage_hint: CaptureStorage::default(),
                })
            })
            .collect()
    }
}

fn capture_mode(ty: &ValkyrieType) -> CaptureMode {
    match ty {
        ValkyrieType::Integer8 { .. }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. }
        | ValkyrieType::Float32
        | ValkyrieType::Float64
        | ValkyrieType::Character
        | ValkyrieType::Boolean
        | ValkyrieType::Unit
        | ValkyrieType::Void => CaptureMode::ByValue,
        _ => CaptureMode::ByReference,
    }
}
