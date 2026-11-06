//! Backend-private physical planning derived from Canonical Semantic MIR.
//!
//! This is deliberately a verifier input, not another executable IR. It
//! records only the physical category required for each already-typed SSA
//! value and exact call identities. Backend emitters must consume this plan;
//! they must not rediscover language semantics from descriptors or stack use.

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{ItemInstanceId, NyarType};

use crate::{
    FragmentSubmission, TargetExecutionPlan,
    backend_plan_views::{ExecutableFunction, ExecutableInstructionKind, ExecutableOperand, ExecutableValueRef},
};

/// Managed physical target selected before backend preparation.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalBackend {
    Jvm,
    Clr,
    WasmCore,
    WasmJsGlue,
    WasiComponent,
}

/// A verifier category, not a source-language type.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalValueCategory {
    Void,
    I32,
    I64,
    F32,
    F64,
    Reference,
}

/// 单个调用的物理参数合同（集成测试专用）。
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCallContract {
    pub callee: ItemInstanceId,
    pub parameters: Vec<PhysicalValueCategory>,
}

/// 文本 ABI 的编码身份：只来自声明的 `NyarType`，不看函数名或包名。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTextEncoding {
    Utf8,
    Utf16,
}

/// 文本 ABI 的宿主边界。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTextBoundary {
    WasmJsGlue,
    WasiComponent,
}

/// 该边界上已经落地的宿主载体。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalTextCarrier {
    /// `utf8` / `utf16`：共享的 i32 JS string 句柄（`hostIntern` / `const_utf8`）。
    /// 句柄不携带编码标签，编码身份由 `PhysicalTextProjection` 记录提供。
    JsStringHandle,
    /// WASI canonical ABI：UTF-8 字节列表偏移。
    CanonicalUtf8Bytes,
}

/// 单个文本 ABI 位置的冻结事实：编码 × 边界 × 载体。
///
/// 同一载体可以被多个编码共用（JS string 句柄不携带编码标签），编码身份只存在于
/// 本记录。`WasiComponent` + `utf16` 不存在 canonical 表示，因此不产生 projection；
/// 编码在边界上没有 carrier 的形态也不产生 projection。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalTextProjection {
    pub encoding: PhysicalTextEncoding,
    pub boundary: PhysicalTextBoundary,
    pub carrier: Option<PhysicalTextCarrier>,
}

/// `utf16` 在所有 wasm 边界上的统一拒绝理由。
#[doc(hidden)]
pub const UTF16_UNRESOLVED_TEXT_ABI: &str =
    "`utf16` 没有已声明的文本 ABI 载体：canonical ABI 的字符串是 UTF-8 字节列表，\
     该边界不存在 `utf16` 的 canonical 表示，必须由显式 UTF-16 文本 ABI 契约提供载体";

/// 宿主边界到文本 ABI 边界的唯一映射。
pub(crate) fn text_abi_boundary(backend: PhysicalBackend) -> Option<PhysicalTextBoundary> {
    match backend {
        PhysicalBackend::WasmJsGlue => Some(PhysicalTextBoundary::WasmJsGlue),
        PhysicalBackend::WasiComponent => Some(PhysicalTextBoundary::WasiComponent),
        PhysicalBackend::Jvm | PhysicalBackend::Clr | PhysicalBackend::WasmCore => None,
    }
}

/// 文本 ABI 事实的唯一查询点：编码来自声明的 `NyarType`，载体来自该边界。
pub(crate) fn declared_text_projection(boundary: PhysicalTextBoundary, ty: &NyarType) -> Option<PhysicalTextProjection> {
    let encoding = match ty {
        NyarType::Utf8 => PhysicalTextEncoding::Utf8,
        NyarType::Utf16 => PhysicalTextEncoding::Utf16,
        _ => return None,
    };
    let carrier = match (encoding, boundary) {
        (PhysicalTextEncoding::Utf8, PhysicalTextBoundary::WasmJsGlue) => Some(PhysicalTextCarrier::JsStringHandle),
        (PhysicalTextEncoding::Utf8, PhysicalTextBoundary::WasiComponent) => Some(PhysicalTextCarrier::CanonicalUtf8Bytes),
        // `WasiComponent` 的 canonical ABI 只有 UTF-8 字节列表，没有 `utf16` 的
        // canonical 表示，因此该边界不存在 `utf16` 的文本 ABI 事实。
        (PhysicalTextEncoding::Utf16, PhysicalTextBoundary::WasiComponent) => return None,
        // `WasmJsGlue` 与 `utf8` 共用同一张 JS string 句柄表：JS 只有一种字符串
        // 类型，`utf8` 与 `utf16` 的差异只在入表前是否解码，句柄本身不携带编码标签。
        (PhysicalTextEncoding::Utf16, PhysicalTextBoundary::WasmJsGlue) => Some(PhysicalTextCarrier::JsStringHandle),
    };
    Some(PhysicalTextProjection { encoding, boundary, carrier })
}

/// 未落地文本 ABI 载体的统一拒绝理由。
pub(crate) fn unresolved_text_abi_reason(ty: &NyarType) -> Option<&'static str> {
    match ty {
        NyarType::Utf16 => Some(UTF16_UNRESOLVED_TEXT_ABI),
        _ => None,
    }
}

#[cfg(test)]
mod text_abi_tests {
    use super::*;

    #[test]
    fn text_abi_fact_binds_encoding_boundary_and_carrier_from_declared_type() {
        let js_utf8 = declared_text_projection(PhysicalTextBoundary::WasmJsGlue, &NyarType::Utf8)
            .expect("`utf8` 必须已有冻结载体");
        assert_eq!(js_utf8.encoding, PhysicalTextEncoding::Utf8);
        assert_eq!(js_utf8.boundary, PhysicalTextBoundary::WasmJsGlue);
        assert_eq!(js_utf8.carrier, Some(PhysicalTextCarrier::JsStringHandle));

        for boundary in [PhysicalTextBoundary::WasmJsGlue, PhysicalTextBoundary::WasiComponent] {
            let projection = declared_text_projection(boundary, &NyarType::Utf16);
            match boundary {
                // `WasmJsGlue` 已冻结编码身份与共享载体，但 lowering 仍未接入该载体。
                PhysicalTextBoundary::WasmJsGlue => {
                    let projection = projection.expect("`WasmJsGlue` 的 `utf16` 编码身份必须已冻结");
                    assert_eq!(projection.encoding, PhysicalTextEncoding::Utf16);
                    assert_eq!(projection.boundary, boundary);
                    assert_eq!(
                        projection.carrier,
                        Some(PhysicalTextCarrier::JsStringHandle),
                        "`utf16` 与 `utf8` 共用一个 JS string 句柄表"
                    );
                }
                // canonical ABI 没有 `utf16` 表示，该边界不存在文本 ABI 事实。
                PhysicalTextBoundary::WasiComponent => assert_eq!(projection, None),
            }
        }
        assert_eq!(
            unresolved_text_abi_reason(&NyarType::Utf16),
            Some(UTF16_UNRESOLVED_TEXT_ABI)
        );
        assert_eq!(unresolved_text_abi_reason(&NyarType::Utf8), None);
        assert_eq!(declared_text_projection(PhysicalTextBoundary::WasmJsGlue, &NyarType::Unit), None);
    }
}

/// 单个函数的物理类别计划（集成测试专用）。
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalFunctionPlan {
    pub symbol: String,
    pub parameters: Vec<PhysicalValueCategory>,
    pub result: PhysicalValueCategory,
    pub values: BTreeMap<ExecutableValueRef, PhysicalValueCategory>,
    pub calls: BTreeMap<(u32, usize), PhysicalCallContract>,
    pub text_projections: BTreeMap<ExecutableValueRef, PhysicalTextProjection>,
}

/// 物理合同拒绝的错误定位（集成测试专用）。
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalPlanError {
    pub code: &'static str,
    pub function: String,
    pub location: String,
    pub detail: String,
}

impl PhysicalPlanError {
    fn new(code: &'static str, function: &ExecutableFunction, location: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { code, function: function.symbol.clone(), location: location.into(), detail: detail.into() }
    }
}

/// Stable, backend-independent observation for the paired emitter/frontend
/// physical contract suite. Backend identities and physical carrier details
/// deliberately do not enter this record.
pub(crate) fn observation(case_id: &str, result: Result<(), &PhysicalPlanError>) -> String {
    match result {
        Ok(()) => format!("{case_id}|accept||"),
        Err(error) => format!("{case_id}|reject|{}|{}", error.code, observation_site(&error.location)),
    }
}

fn observation_site(location: &str) -> &'static str {
    if location == "entry" {
        "entry"
    }
    else if location.contains("instruction") {
        "instruction"
    }
    else if location.contains("block") {
        "block"
    }
    else if location.contains("function") || location.contains("value") || location.contains("parameter") {
        "function"
    }
    else {
        "module"
    }
}

/// Build a backend-private physical plan without consulting backend artifacts,
/// descriptors, local slots, or host carriers.
/// 为给定片段与目标生成逐函数物理类别计划（集成测试专用）。
#[doc(hidden)]
pub fn build_physical_plan(submission: &FragmentSubmission, _backend: PhysicalBackend) -> Result<Vec<PhysicalFunctionPlan>, PhysicalPlanError> {
    let executable = &submission.backend_plan;
    executable
        .instances()
        .into_iter()
        .map(|operation| {
            let view = executable.get_function(&operation).ok_or_else(|| PhysicalPlanError {
                code: "BPHYS004",
                function: operation.to_string(),
                location: "function".to_string(),
                detail: "physical planning requires an exact executable function".to_string(),
            })?;
            build_function_plan(executable.as_ref(), &view.function, _backend)
        })
        .collect()
}

/// Require a complete physical plan before any backend-local preparation.
///
/// The plan is intentionally discarded here. Individual backends will consume
/// their own plan in later migration steps; this gate first makes incomplete
/// legacy submissions fail closed at the common boundary.
pub(crate) fn validate_physical_submission(submission: &FragmentSubmission, backend: PhysicalBackend) -> Result<(), PhysicalPlanError> {
    let plans = build_physical_plan(submission, backend)?;
    if let Some(entry) = submission.backend_plan.entry_operation() {
        let executable = &submission.backend_plan;
        if executable.get_function(&entry).is_none() {
            return Err(PhysicalPlanError {
                code: "BPHYS008",
                function: entry.to_string(),
                location: "entry".to_string(),
                detail: "entry projection has no exact semantic function".to_string(),
            });
        }
    }
    let _ = plans;
    Ok(())
}

fn build_function_plan(
    executable: &TargetExecutionPlan,
    function: &ExecutableFunction,
    backend: PhysicalBackend,
) -> Result<PhysicalFunctionPlan, PhysicalPlanError> {
    let (text_values, text_projections) = collect_text_projections(function, backend)?;
    let parameters = function
        .param_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            physical_category(backend, ty, false, text_values.contains(&crate::contracts::ValueRef(index as u32)), function, "function")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let return_text_authorized = function.blocks.iter().any(|block| {
        matches!(
            block.terminator,
            crate::contracts::Terminator::Return { value: Some(ExecutableOperand::Value(value)) } if text_values.contains(&value)
        )
    });
    let result = physical_category(backend, &function.return_type, true, return_text_authorized, function, "function")?;
    let values = function
        .value_types
        .iter()
        .map(|(value, ty)| {
            physical_category(backend, ty, false, text_values.contains(value), function, format!("value {}", value.0))
                .map(|category| (*value, category))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut calls = BTreeMap::new();
    for block in &function.blocks {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let ExecutableInstructionKind::Call { callee, arguments, .. } = &instruction.kind
            else {
                continue;
            };
            let location = format!("block {} instruction {instruction_index}", block.id.0);
            let callee = match callee {
                ExecutableOperand::Item(instance) => *instance,
                // batch 7：语言 intrinsic 不再以 Symbol path 到达 physical lowering。
                // intrinsic 调用必须在 Semantic MIR 降级为 Call { callee: Item(instance) }
                // 才能通过 adaptor/import binding 消费。
                ExecutableOperand::Symbol(_) => {
                    return Err(PhysicalPlanError::new(
                        "BPHYS004",
                        function,
                        location.clone(),
                        "ordinary callable must carry a resolved item identity; intrinsic calls must degrade to Call with Item(instance) in Semantic MIR",
                    ));
                }
                ExecutableOperand::Value(_) | ExecutableOperand::Constant(_) => continue,
            };
            let callee_param_types = resolve_callee_parameter_types(executable, callee, function, &location)?;
            let parameters = arguments
                .iter()
                .enumerate()
                .map(|(arg_index, arg)| {
                    let owned = match arg {
                        ExecutableOperand::Value(v) => function.value_types.get(v).cloned().ok_or_else(|| {
                            PhysicalPlanError::new("BPHYS004", function, location.clone(), "call argument missing semantic type")
                        })?,
                        ExecutableOperand::Constant(constant) => constant_nyar_type(constant),
                        // 调用点残留的裸 Symbol（如未类型化的 `None`）不得再当成 Unit；
                        // 物理类别以 callee 精确形参合同为准，不向上猜语义。
                        ExecutableOperand::Symbol(_) => callee_param_types.get(arg_index).cloned().ok_or_else(|| {
                            PhysicalPlanError::new(
                                "BPHYS004",
                                function,
                                location.clone(),
                                format!("bare symbol argument has no callee parameter type at index {arg_index}"),
                            )
                        })?,
                        ExecutableOperand::Item(_) => {
                            return Err(PhysicalPlanError::new(
                                "BPHYS004",
                                function,
                                location.clone(),
                                "callable identity cannot be used as a call argument",
                            ));
                        }
                    };
                    physical_category(
                        backend,
                        &owned,
                        false,
                        false,
                        function,
                        format!("call parameter[{arg_index}] callee={callee} ty={owned:?}"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            calls.insert((block.id.0, instruction_index), PhysicalCallContract { callee, parameters });
        }
    }
    Ok(PhysicalFunctionPlan { symbol: function.symbol.clone(), parameters, result, values, calls, text_projections })
}

fn resolve_callee_parameter_types(
    executable: &TargetExecutionPlan,
    callee: ItemInstanceId,
    function: &ExecutableFunction,
    location: &str,
) -> Result<Vec<NyarType>, PhysicalPlanError> {
    if let Some(view) = executable.get_function(&callee) {
        return Ok(view.function.param_types.clone());
    }
    if let Some(import) = executable.imports().get(&callee) {
        return Ok(import.parameter_types.clone());
    }
    Err(PhysicalPlanError::new(
        "BPHYS004",
        function,
        location.clone(),
        "call identity is not an exact local semantic function or declared import",
    ))
}

/// Map an executable immediate to its Semantic MIR type for physical planning.
fn constant_nyar_type(constant: &crate::contracts::Constant) -> NyarType {
    match constant {
        crate::contracts::Constant::Int(_) => NyarType::Integer64 { signed: true },
        crate::contracts::Constant::Float64(_) => NyarType::Float64,
        crate::contracts::Constant::Bool(_) => NyarType::Boolean,
        crate::contracts::Constant::Utf8(_) => NyarType::Utf8,
        crate::contracts::Constant::Utf16(_) => NyarType::Utf16,
        crate::contracts::Constant::Unit => NyarType::Unit,
        // 空值没有独立名义类型；它只能经带 `T?` 类型的 LoadConstant 进入计划。
        crate::contracts::Constant::Null => panic!("bare null constant has no standalone semantic type"),
    }
}

fn collect_text_projections(
    function: &ExecutableFunction,
    backend: PhysicalBackend,
) -> Result<(BTreeSet<ExecutableValueRef>, BTreeMap<ExecutableValueRef, PhysicalTextProjection>), PhysicalPlanError> {
    // Utf8/Utf16 identity lives on NyarType. 编码 × 边界 × 载体只在这里冻结，
    // 物理落点不得各自重新推断。
    let Some(boundary) = text_abi_boundary(backend) else {
        return Ok((BTreeSet::new(), BTreeMap::new()));
    };
    let projections: BTreeMap<ExecutableValueRef, PhysicalTextProjection> = function
        .value_types
        .iter()
        .filter_map(|(value, ty)| declared_text_projection(boundary, ty).map(|projection| (*value, projection)))
        .collect();
    let text_values = projections.keys().copied().collect();
    Ok((text_values, projections))
}

fn physical_category(
    _backend: PhysicalBackend,
    ty: &NyarType,
    is_result: bool,
    text_authorized: bool,
    function: &ExecutableFunction,
    location: impl Into<String>,
) -> Result<PhysicalValueCategory, PhysicalPlanError> {
    let category = match ty {
        NyarType::Bottom | NyarType::Unit if is_result => PhysicalValueCategory::Void,
        NyarType::Bottom | NyarType::Unit => {
            return Err(PhysicalPlanError::new("BPHYS001", function, location, "void/unit cannot occupy a physical value slot"));
        }
        NyarType::Boolean | NyarType::Character | NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => {
            PhysicalValueCategory::I32
        }
        NyarType::Integer64 { .. } => PhysicalValueCategory::I64,
        NyarType::Float32 => PhysicalValueCategory::F32,
        NyarType::Float64 => PhysicalValueCategory::F64,
        NyarType::Integer128 { .. } => {
            return Err(PhysicalPlanError::new("BPHYS001", function, location, "i128 has no declared managed physical category"));
        }
        // Utf8 vs Utf16 remain distinct NyarType identities (no TextConvert God opcode).
        NyarType::Utf8 | NyarType::Utf16 => {
            let _ = text_authorized;
            PhysicalValueCategory::Reference
        }
        NyarType::Named(_)
        | NyarType::Nominal { .. }
        | NyarType::Apply(_, _)
        | NyarType::Function(_)
        | NyarType::Tuple(_)
        | NyarType::Array(_)
        | NyarType::FixedArray { .. }
        | NyarType::TraitObject(_)
        | NyarType::Nullable(_)
        | NyarType::Union(_) => PhysicalValueCategory::Reference,
    };
    Ok(category)
}
