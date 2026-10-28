use std::collections::BTreeMap;

use nyar_types::{
    CanonicalMirError, CanonicalOperation, CanonicalProgram, InstructionId, ItemId, ItemInstanceId, MirValueId, StructuredDiagnosticSet, TypeId,
};

use crate::compiler::{
    mir::{MirFunction, MirModule},
    types::hir::ValkyrieType,
};

use crate::compiler::emitter::pipeline::diagnostics::fail_stage;

pub(crate) fn ensure_instantiated_signature(function: &MirFunction) -> Result<(), StructuredDiagnosticSet> {
    if function
        .param_types
        .iter()
        .chain(std::iter::once(&function.return_type))
        .any(crate::compiler::mir::ssa::type_contains_non_monomorphic_signature_reference)
    {
        Err(error_without_module("CAN013", format!("函数 `{}` 签名仍包含 Self/关联类型占位", function.display_symbol)))
    }
    else {
        Ok(())
    }
}
pub(crate) fn type_id(ids: &BTreeMap<ValkyrieType, TypeId>, ty: &ValkyrieType) -> Result<TypeId, StructuredDiagnosticSet> {
    ids.get(ty)
        .copied()
        .or_else(|| {
            // Named primitive 可能已经在 type_identity_table 里被归一到底层变体；
            // 这里做 fallback normalize 查找。
            let normalized = canonical_normalize_named_primitive(ty);
            if &normalized != ty { ids.get(&normalized).copied() } else { None }
        })
        .ok_or_else(|| error_without_module("CAN014", format!("类型事实未进入 canonical table: {ty:?}")))
}

/// 把 primitive numeric 的 Named surface name 归一到底层 primitive 变体。
/// 其他类型原样返回。
fn canonical_normalize_named_primitive(ty: &ValkyrieType) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) => match name.as_str() {
            "boolean" | "Bool" | "bool" => ValkyrieType::Boolean,
            "i8" => ValkyrieType::Integer8 { signed: true },
            "u8" | "byte" => ValkyrieType::Integer8 { signed: false },
            "i16" => ValkyrieType::Integer16 { signed: true },
            "u16" => ValkyrieType::Integer16 { signed: false },
            "i32" | "int" | "Int" => ValkyrieType::Integer32 { signed: true },
            "u32" | "uint" | "UInt" => ValkyrieType::Integer32 { signed: false },
            "i64" | "long" | "Long" => ValkyrieType::Integer64 { signed: true },
            "u64" | "ulong" => ValkyrieType::Integer64 { signed: false },
            "i128" => ValkyrieType::Integer128 { signed: true },
            "u128" => ValkyrieType::Integer128 { signed: false },
            "f32" | "float32" => ValkyrieType::Float32,
            "f64" | "float" | "Float" | "double" | "Double" => ValkyrieType::Float64,
            _ => ty.clone(),
        },
        _ => ty.clone(),
    }
}
fn item_id(index: u32) -> ItemId {
    ItemId::from_index(index).expect("item identity overflow")
}
pub(crate) fn error_without_module(code: &'static str, message: impl Into<String>) -> StructuredDiagnosticSet {
    fail_stage::<()>(nyar_types::CompileStage::SemanticMir, code, "", message).err().unwrap()
}
pub(crate) fn error(module: &MirModule, code: &'static str, message: impl Into<String>) -> StructuredDiagnosticSet {
    fail_stage::<()>(nyar_types::CompileStage::SemanticMir, code, &module.name, message).err().unwrap()
}
pub(crate) fn canonical_error(module: &MirModule, program: &CanonicalProgram, error: CanonicalMirError) -> StructuredDiagnosticSet {
    let detail = match &error {
        CanonicalMirError::OperationResultTypeMismatch { function, instruction } => {
            format!("canonical MIR 校验失败: {error:?}{}", describe_operation_result_mismatch(module, program, *function, *instruction))
        }
        CanonicalMirError::CallResultTypeMismatch { function, callee, value } => {
            format!("canonical MIR 校验失败: {error:?}{}", describe_call_result_mismatch(module, program, *function, *callee, *value))
        }
        _ => format!("canonical MIR 校验失败: {error:?}"),
    };
    fail_stage::<()>(nyar_types::CompileStage::ValidateMir, "CAN015", &module.name, detail).err().unwrap()
}

fn describe_call_result_mismatch(
    module: &MirModule,
    program: &CanonicalProgram,
    function: ItemInstanceId,
    callee: ItemInstanceId,
    value: MirValueId,
) -> String {
    let caller =
        module.functions.iter().find(|item| item.instance == Some(function)).map(|item| item.display_symbol.as_str()).unwrap_or("<unknown>");
    let callee_symbol = module
        .functions
        .iter()
        .find(|item| item.instance == Some(callee))
        .map(|item| item.display_symbol.clone())
        .or_else(|| program.linked.callable_names.get(&callee).map(|name| name.to_string()))
        .unwrap_or_else(|| "<unknown>".to_string());
    let actual = program
        .mir
        .functions
        .get(&function)
        .and_then(|func| func.value_types.get(&value))
        .and_then(|ty| program.linked.types.get(ty).map(|row| (&row.kind, *ty)));
    let expected = program.linked.item_instances.get(&callee).map(|record| {
        let kind = program.linked.types.get(&record.return_type).map(|row| &row.kind);
        (kind, record.return_type)
    });
    format!(" (caller=`{caller}` callee=`{callee_symbol}` actual={actual:?} expected_return={expected:?})")
}

fn describe_operation_result_mismatch(
    module: &MirModule,
    program: &CanonicalProgram,
    function: ItemInstanceId,
    instruction: InstructionId,
) -> String {
    let symbol = module
        .functions
        .iter()
        .find(|item| item.instance == Some(function))
        .map(|item| item.display_symbol.clone())
        .or_else(|| program.linked.callable_names.get(&function).map(|name| name.to_string()))
        .unwrap_or_else(|| "<unknown>".to_string());
    let Some(func) = program.mir.functions.get(&function)
    else {
        return format!(" (function_symbol=`{symbol}`)");
    };
    for block in func.blocks.values() {
        for instr in &block.instructions {
            if instr.id != instruction {
                continue;
            }
            let describe = |value: &MirValueId| -> String {
                let ty = func.value_types.get(value);
                let kind = ty.and_then(|id| program.linked.types.get(id)).map(|row| &row.kind);
                format!("{value:?}->{ty:?}/{kind:?}")
            };
            let result_types: Vec<_> = instr.results.iter().map(describe).collect();
            let operand_hint = match &instr.operation {
                CanonicalOperation::Copy { source } => format!(" source={}", describe(source)),
                _ => String::new(),
            };
            return format!(" (function_symbol=`{symbol}` op={:?}{operand_hint} results=[{}])", instr.operation, result_types.join(", "));
        }
    }
    format!(" (function_symbol=`{symbol}`)")
}
