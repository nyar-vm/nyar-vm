//! 值语义聚合复制的唯一分类点。
//!
//! 语言合同：`structure` / 值语义 `unite` 按值复制，嵌套值聚合必须逐层产生
//! 新对象；`class`、数组、闭包、引用语义 sum 等按引用共享。判定只消费冻结
//! 布局事实（`StorageKind::Value` 且字段非空），不按类型名、字段名或定义链猜。
//!
//! 各后端只消费本模块给出的分类，并保留自己的对象分配与字段编码。

use nyar_types::{AggregateLayout, AggregateLayoutPlan, LayoutId, NyarType, StorageKind};

/// 单次值聚合复制允许的最大嵌套深度。
///
/// 值语义聚合按值内联，类型上不可能形成无限递归；该上限只用于把损坏的布局
/// 事实变成结构化失败，而不是耗尽编译器栈。
pub(crate) const MAX_VALUE_AGGREGATE_COPY_DEPTH: u32 = 32;

/// 字段类型是否为需要递归复制的嵌套值聚合。
///
/// 返回嵌套布局身份；标量、引用语义聚合、空聚合与数组/闭包等返回 `None`
/// （按引用共享或本来就按值搬运）。
pub(crate) fn nested_value_aggregate_layout(plan: &AggregateLayoutPlan, ty: &NyarType) -> Option<LayoutId> {
    let NyarType::Nominal { layout, .. } = ty
    else {
        return None;
    };
    let candidate = plan.layouts.iter().find(|candidate| candidate.id == *layout)?;
    (candidate.storage == StorageKind::Value && !candidate.fields.is_empty()).then_some(*layout)
}

/// 该布局的值聚合复制需要的最深嵌套层数（顶层布局为 0）。
///
/// 后端据此预留临时引用槽与递归预算；超过 [`MAX_VALUE_AGGREGATE_COPY_DEPTH`]
/// 的布局事实视为损坏，由调用方结构化拒绝。
pub(crate) fn value_aggregate_copy_depth(plan: &AggregateLayoutPlan, layout: &AggregateLayout) -> u32 {
    fn walk(plan: &AggregateLayoutPlan, layout: &AggregateLayout, depth: u32) -> u32 {
        let mut max_depth = depth;
        for field in &layout.fields {
            let Some(nested_id) = nested_value_aggregate_layout(plan, &field.ty)
            else {
                continue;
            };
            let Some(nested) = plan.layouts.iter().find(|candidate| candidate.id == nested_id)
            else {
                continue;
            };
            max_depth = max_depth.max(walk(plan, nested, depth + 1));
        }
        max_depth
    }
    walk(plan, layout, 0)
}
