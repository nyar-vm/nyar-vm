//! Concretize Valkyrie HIR types into platform [`NyarType`].

use std::{collections::BTreeMap, fmt};

use nyar::{NyarFunctionType, NyarType, WitnessObject};

use crate::compiler::{
    mir::{MirFunction, MirValueRef},
    types::hir::ValkyrieType,
};

/// Failure while turning a frontend type into a platform type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConcretizeError {
    /// Human-readable reason.
    pub message: String,
}

impl ConcretizeError {
    fn new(message: impl Into<String>) -> Self {
        Self { message: message.into() }
    }

    /// 附上布局/声明位置，供结构化诊断定位；不改错误类别，只补上下文。
    pub(crate) fn at(mut self, location: impl fmt::Display) -> Self {
        self.message = format!("{} (at {location})", self.message);
        self
    }
}

impl fmt::Display for ConcretizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ConcretizeError {}

/// Expand fixed-width numeric spellings (`i32`, …) and text carriers
/// (`Utf8Text`/`utf8`, `Utf16Text`/`utf16`) to concrete platform shapes.
///
/// `Named("usize")`/`Named("isize")` deliberately fall through to a structured error in
/// [`concretize_type`]: their width is target-dependent and must be settled by the
/// `std::primitive::pointer_width` alias expansion, never guessed as `Integer32`.
/// `type utf16 = Utf16Text` leaves `return self` as `Named(Utf16Text)` while the declared
/// return is `ValkyrieType::Utf16` → SMIR007 unless both concretize to `NyarType::Utf16`.
/// 指针宽度整数面（`usize`/`isize` 及其限定拼写）：宽度由目标决定，不能由短名给载体。
fn is_platform_width_surface_name(name: &str) -> bool {
    matches!(name.rsplit([':', '.']).next().unwrap_or(name), "usize" | "isize")
}

fn concretize_primitive_named_alias(name: &str) -> Option<NyarType> {
    let simple = name.rsplit([':', '.']).next().unwrap_or(name);
    Some(match simple {
        "bool" => NyarType::Boolean,
        "u8" => NyarType::Integer8 { signed: false },
        "i8" => NyarType::Integer8 { signed: true },
        "u16" => NyarType::Integer16 { signed: false },
        "i16" => NyarType::Integer16 { signed: true },
        // 定宽整数按声明拼写给载体；`usize`/`isize` 是目标相关宽度，必须由
        // `std::primitive::pointer_width` 的别名展开结算，禁止在此按名字猜位宽。
        "u32" | "i32" => NyarType::Integer32 { signed: true },
        "u64" | "i64" => NyarType::Integer64 { signed: true },
        "u128" | "i128" => NyarType::Integer128 { signed: true },
        "f32" => NyarType::Float32,
        "f64" => NyarType::Float64,
        "utf8" | "Utf8Text" => NyarType::Utf8,
        "utf16" | "Utf16Text" => NyarType::Utf16,
        // `f128` 没有 carrier：宁可失败关闭，也不静默降精度成 `Float64`。
        _ => return None,
    })
}

/// Strict concretize: rejects frontend-only constructs that must not reach backends.
pub fn concretize_type(ty: &ValkyrieType) -> Result<NyarType, ConcretizeError> {
    match ty {
        ValkyrieType::Void => Ok(NyarType::Bottom),
        ValkyrieType::Unit => Ok(NyarType::Unit),
        ValkyrieType::Boolean => Ok(NyarType::Boolean),
        ValkyrieType::Integer8 { signed } => Ok(NyarType::Integer8 { signed: *signed }),
        ValkyrieType::Integer16 { signed } => Ok(NyarType::Integer16 { signed: *signed }),
        ValkyrieType::Integer32 { signed } => Ok(NyarType::Integer32 { signed: *signed }),
        ValkyrieType::Integer64 { signed } => Ok(NyarType::Integer64 { signed: *signed }),
        ValkyrieType::Integer128 { signed } => Ok(NyarType::Integer128 { signed: *signed }),
        ValkyrieType::Float32 => Ok(NyarType::Float32),
        ValkyrieType::Float64 => Ok(NyarType::Float64),
        ValkyrieType::Character => Ok(NyarType::Character),
        ValkyrieType::Utf8 => Ok(NyarType::Utf8),
        ValkyrieType::Utf16 => Ok(NyarType::Utf16),
        // `Named("Self")` 是未代入污点，与 `SelfType` 同等失败关闭；禁止进 executable。
        ValkyrieType::Named(name) if name.as_str() == "Self" => {
            Err(ConcretizeError::new("Named(\"Self\") must be substituted before platform lowering"))
        }
        // 指针宽度别名缺声明时必须失败关闭：静默按 32 位会产出错误的字段载体与布局。
        ValkyrieType::Named(name) if is_platform_width_surface_name(name.as_str()) => Err(ConcretizeError::new(format!(
            "platform-width type `{name}` must be resolved to a fixed-width integer by pointer-width alias expansion before platform lowering"
        ))),
        ValkyrieType::Named(name) => Ok(concretize_primitive_named_alias(name.as_str()).unwrap_or_else(|| NyarType::Named(name.clone()))),
        ValkyrieType::QualifiedPath { namespace, name } => Err(ConcretizeError::new(format!(
            "qualified type path `{}` must bind to Nominal before platform lowering",
            ValkyrieType::qualified_path_label(namespace, name)
        ))),
        // 平台层尚未携带 ItemId 名义形状：成功路径不得把 declaration 压成短名 Named。
        ValkyrieType::Nominal { declaration, .. } => {
            Err(ConcretizeError::new(format!("Nominal declaration `{declaration:?}` 需要平台 NyarType::Nominal，禁止降级为 Named 短名")))
        }
        ValkyrieType::Apply(base, args) => {
            // 禁止 `Named("Array")` 短名转结构数组；须已是 `ValkyrieType::Array` 或 Nominal 身份。
            if args.len() == 1 && matches!(base.as_ref(), ValkyrieType::Named(name) if name.as_str() == "Array") {
                return Err(ConcretizeError::new(
                    "Apply(Named(\"Array\"), …) 不得降为平台 Array；须使用结构化 Array 类型或 Nominal declaration",
                ));
            }
            Ok(NyarType::Apply(Box::new(concretize_type(base)?), args.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?))
        }
        ValkyrieType::Function(func) => Ok(NyarType::Function(Box::new(NyarFunctionType {
            params: func.params.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?,
            return_type: concretize_type(&func.return_type)?,
        }))),
        ValkyrieType::Tuple(elems) => Ok(NyarType::Tuple(elems.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?)),
        ValkyrieType::Array(elem) => Ok(NyarType::Array(Box::new(concretize_type(elem)?))),
        ValkyrieType::FixedArray { element, length } => {
            Ok(NyarType::FixedArray { element: Box::new(concretize_type(element)?), length: *length })
        }
        ValkyrieType::TraitObject(object) => Ok(NyarType::TraitObject(WitnessObject {
            trait_path: object.trait_path.clone(),
            type_arguments: object.type_arguments.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?,
        })),
        ValkyrieType::Nullable(payload) => Ok(NyarType::Nullable(Box::new(concretize_type(payload)?))),
        ValkyrieType::Union(arms) => Ok(NyarType::Union(arms.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?)),
        ValkyrieType::AutoType => Err(ConcretizeError::new("auto type must be resolved before platform lowering")),
        ValkyrieType::SelfType => Err(ConcretizeError::new("Self type must be substituted before platform lowering")),
        ValkyrieType::Generic(generic) => {
            Err(ConcretizeError::new(format!("generic parameter `{}` must be instantiated before platform lowering", generic.name)))
        }
        ValkyrieType::TypeLambda(_) => Err(ConcretizeError::new("type lambda must be erased before platform lowering")),
        ValkyrieType::Associated(_) => Err(ConcretizeError::new("associated type must be resolved before platform lowering")),
        ValkyrieType::Row(_) => Err(ConcretizeError::new("row type must be erased before platform lowering")),
        ValkyrieType::Intersection(_) => Err(ConcretizeError::new("intersection type must be erased before platform lowering")),
    }
}

/// Concretize MIR function type maps into platform types.
pub fn concretize_mir_function_types(
    function: &MirFunction,
) -> Result<(NyarType, Vec<NyarType>, BTreeMap<MirValueRef, NyarType>), ConcretizeError> {
    let return_type = concretize_type(&function.return_type)?;
    let param_types = function.param_types.iter().map(concretize_type).collect::<Result<Vec<_>, _>>()?;
    let mut value_types = BTreeMap::new();
    for (key, ty) in &function.value_types {
        value_types.insert(*key, concretize_type(ty)?);
    }
    Ok((return_type, param_types, value_types))
}
