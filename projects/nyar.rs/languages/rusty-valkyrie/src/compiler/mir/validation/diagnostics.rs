//! 类型显示与 effect 参数合同辅助。

use crate::compiler::{mir::MirEffectKind, types::hir::ValkyrieType};

pub(super) fn display_type(ty: &ValkyrieType) -> String {
    match ty {
        ValkyrieType::Void => "void".to_string(),
        ValkyrieType::Unit => "unit".to_string(),
        ValkyrieType::Boolean => "bool".to_string(),
        ValkyrieType::Integer8 { signed } => integer_type_name(*signed, 8),
        ValkyrieType::Integer16 { signed } => integer_type_name(*signed, 16),
        ValkyrieType::Integer32 { signed } => integer_type_name(*signed, 32),
        ValkyrieType::Integer64 { signed } => integer_type_name(*signed, 64),
        ValkyrieType::Integer128 { signed } => integer_type_name(*signed, 128),
        ValkyrieType::Float32 => "f32".to_string(),
        ValkyrieType::Float64 => "f64".to_string(),
        ValkyrieType::Character => "char".to_string(),
        ValkyrieType::Utf8 => "utf8".to_string(),
        ValkyrieType::Utf16 => "utf16".to_string(),
        ValkyrieType::Named(name) => name.to_string(),
        ValkyrieType::QualifiedPath { namespace, name } => ValkyrieType::qualified_path_label(namespace, name),
        ValkyrieType::Nominal { declaration, arguments, .. } => {
            if arguments.is_empty() {
                format!("Nominal({declaration:?})")
            }
            else {
                format!("Nominal({declaration:?})<{}>", arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
            }
        }
        ValkyrieType::Apply(base, arguments) => {
            format!("{}<{}>", display_type(base), arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
        }
        ValkyrieType::Generic(generic) => generic.name.to_string(),
        ValkyrieType::Function(function) => format!(
            "micro({}) -> {}",
            function.params.iter().map(display_type).collect::<Vec<_>>().join(", "),
            display_type(&function.return_type)
        ),
        ValkyrieType::Tuple(items) => format!("({})", items.iter().map(display_type).collect::<Vec<_>>().join(", ")),
        ValkyrieType::LabeledTuple(fields) => format!(
            "({})",
            fields.iter().map(|field| format!("{}: {}", field.name, display_type(&field.ty))).collect::<Vec<_>>().join(", ")
        ),
        ValkyrieType::Row(row) => format!(
            "{{ {} }}",
            row.methods
                .iter()
                .map(|method| {
                    format!(
                        "{}({}) -> {}",
                        method.name,
                        method.params.iter().map(display_type).collect::<Vec<_>>().join(", "),
                        display_type(&method.return_type)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ValkyrieType::Array(item) => format!("[{}]", display_type(item)),
        ValkyrieType::FixedArray { element, length } => {
            format!("[{}; {}]", display_type(element), length)
        }
        ValkyrieType::TypeLambda(lambda) => format!(
            "type lambda({}) -> {}",
            lambda.params.iter().map(|item| item.name.to_string()).collect::<Vec<_>>().join(", "),
            display_type(&lambda.body)
        ),
        ValkyrieType::TraitObject(object) => {
            format!("{}<{}>", object.trait_path, object.type_arguments.iter().map(display_type).collect::<Vec<_>>().join(", "))
        }
        ValkyrieType::Associated(associated) => {
            format!("{}::{}", display_type(&associated.base), associated.name)
        }
        ValkyrieType::AutoType => "auto".to_string(),
        ValkyrieType::SelfType => "Self".to_string(),
        ValkyrieType::Nullable(payload) => format!("{}?", display_type(payload)),
        ValkyrieType::Union(items) => items.iter().map(display_type).collect::<Vec<_>>().join(" | "),
        ValkyrieType::Intersection(items) => items.iter().map(display_type).collect::<Vec<_>>().join(" & "),
    }
}

pub(super) fn integer_type_name(signed: bool, bits: u16) -> String {
    if signed { format!("i{bits}") } else { format!("u{bits}") }
}

/// `Raise` 的 resume 块形参数允许 0 或 1：
/// - 0：`Resume = !`（never）的恢复通道不回灌值，lowering 不声明形参；
/// - 1：handler / 基本类型载体的恢复通道回灌一个值。
/// 其余 effect 仍是定值：AsyncSpawn 为 0，其余为 1。
pub(super) fn expected_effect_resume_parameter_count(effect: MirEffectKind) -> Option<usize> {
    match effect {
        MirEffectKind::AsyncSpawn => Some(0),
        MirEffectKind::Raise => None,
        MirEffectKind::Yield | MirEffectKind::DelegateYield | MirEffectKind::Await | MirEffectKind::AsyncBlock => Some(1),
    }
}

pub(super) fn payload_required(effect: MirEffectKind) -> bool {
    let _ = effect;
    true
}
