//! Valkyrie MIR 的 M2 验证器。
//!
//! 本模块只拒绝不合法 MIR：验证 stable-ID 覆盖、SSA 定义与支配关系、
//! 调用与聚合合同。它不创建、不修正 callable、layout 或类型等语义事实。

use nyar_types::{
    CanonicalPrimitiveType, CanonicalTypeKind, Identifier, LinkedSemanticProgram, NyarType, TypeId,
    semantic_ids::{ItemId, MirValueId},
};
use std::collections::BTreeMap;

use super::operation::{
    ValkyrieMir, ValkyrieMirArrayInitialization, ValkyrieMirCallee, ValkyrieMirConstant, ValkyrieMirEffectKind, ValkyrieMirError,
    ValkyrieMirOperation, ValkyrieMirOperand, ValkyrieMirTerminator,
};

impl ValkyrieMir {
    /// 在进入 RepresentationPlan 前验证 stable-ID、链接和 SSA 合同。
    pub fn validate(&self, linked: &LinkedSemanticProgram) -> Result<(), ValkyrieMirError> {
        validate_linked_types(linked)?;
        let nominal_types = linked.nominal_instances.values().map(|record| record.ty).collect();
        let mut export_names = std::collections::BTreeSet::new();
        for (function, export) in &linked.exports {
            if !linked.item_instances.contains_key(function) || !self.functions.contains_key(function) {
                return Err(ValkyrieMirError::MissingSurfaceBody { function: *function });
            }
            if !export_names.insert(&export.exported_name) {
                return Err(ValkyrieMirError::DuplicateExportName { name: export.exported_name.clone() });
            }
        }
        for function in linked.entries.keys() {
            if !linked.item_instances.contains_key(function) || !self.functions.contains_key(function) {
                return Err(ValkyrieMirError::MissingSurfaceBody { function: *function });
            }
        }
        for (key, function) in &self.functions {
            if key != &function.instance {
                return Err(ValkyrieMirError::FunctionKeyMismatch { key: *key, instance: function.instance });
            }
            let Some(owner) = linked.item_instances.get(key)
            else {
                return Err(ValkyrieMirError::UnknownFunction { function: *key });
            };
            if function.parameters.iter().map(|(_, ty)| *ty).collect::<Vec<_>>() != owner.parameter_types
                || function.return_type != owner.return_type
            {
                return Err(ValkyrieMirError::FunctionSignatureMismatch { function: *key });
            }
            for ty in function.value_types.values() {
                if !linked.types.contains_key(ty) {
                    return Err(ValkyrieMirError::UnknownType { function: *key, ty: *ty });
                }
                validate_nominal_value_type(linked, *ty, &nominal_types)?;
            }
            let mut defined = std::collections::BTreeSet::new();
            for (value, ty) in &function.parameters {
                if !linked.types.contains_key(ty) {
                    return Err(ValkyrieMirError::UnknownType { function: *key, ty: *ty });
                }
                if !defined.insert(*value) {
                    return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *value });
                }
                if function.value_types.get(value) != Some(ty) {
                    return Err(ValkyrieMirError::ValueTypeMismatch { function: *key, value: *value, expected: *ty });
                }
            }
            if !linked.types.contains_key(&function.return_type) {
                return Err(ValkyrieMirError::UnknownType { function: *key, ty: function.return_type });
            }
            if !function.blocks.contains_key(&function.entry) {
                return Err(ValkyrieMirError::UnknownBlock { function: *key, block: function.entry });
            }
            let all_blocks = function.blocks.keys().copied().collect::<std::collections::BTreeSet<_>>();
            let mut predecessors = all_blocks.iter().map(|block| (*block, std::collections::BTreeSet::new())).collect::<BTreeMap<_, _>>();
            for block in function.blocks.values() {
                let targets = match &block.terminator {
                    ValkyrieMirTerminator::Jump { target, .. } => vec![*target],
                    ValkyrieMirTerminator::Branch { then_target, else_target, .. } => vec![*then_target, *else_target],
                    ValkyrieMirTerminator::PerformEffect { resume_target, .. } => {
                        vec![*resume_target]
                    }
                    ValkyrieMirTerminator::StateDispatch { cases, default_target, .. } => {
                        cases.iter().map(|(_, target)| *target).chain(std::iter::once(*default_target)).collect()
                    }
                    _ => Vec::new(),
                };
                for target in targets {
                    if let Some(preds) = predecessors.get_mut(&target) {
                        preds.insert(block.id);
                    }
                }
            }
            let mut dominators = all_blocks
                .iter()
                .map(|block| {
                    let initial = if *block == function.entry { std::collections::BTreeSet::from([*block]) } else { all_blocks.clone() };
                    (*block, initial)
                })
                .collect::<BTreeMap<_, _>>();
            loop {
                let mut changed = false;
                for block in all_blocks.iter().copied().filter(|block| *block != function.entry) {
                    let mut next = all_blocks.clone();
                    for predecessor in predecessors.get(&block).into_iter().flat_map(|values| values.iter()) {
                        next = next.intersection(dominators.get(predecessor).unwrap()).copied().collect();
                    }
                    next.insert(block);
                    if next != dominators[&block] {
                        dominators.insert(block, next);
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
            let mut definitions = function.parameters.iter().map(|(value, _)| (*value, None)).collect::<BTreeMap<_, _>>();
            for block in function.blocks.values() {
                for (value, _) in &block.parameters {
                    if block.id == function.entry {
                        if !function.parameters.iter().any(|(parameter, _)| parameter == value) {
                            return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *value });
                        }
                        continue;
                    }
                    if definitions.insert(*value, Some(block.id)).is_some() {
                        return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *value });
                    }
                }
                for value in block.instructions.iter().flat_map(|instruction| instruction.results.iter()) {
                    if definitions.insert(*value, Some(block.id)).is_some() {
                        return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *value });
                    }
                }
            }
            for (block_id, block) in &function.blocks {
                let mut block_defined = defined.clone();
                let value_is_available = |value: &MirValueId| match definitions.get(value) {
                    Some(None) => true,
                    Some(Some(definition_block)) => *definition_block == *block_id || dominators[block_id].contains(definition_block),
                    None => false,
                };
                for (index, (value, ty)) in block.parameters.iter().enumerate() {
                    if !linked.types.contains_key(ty) {
                        return Err(ValkyrieMirError::UnknownType { function: *key, ty: *ty });
                    }
                    if block.id == function.entry {
                        if function.parameters.get(index) != Some(&(*value, *ty)) {
                            return Err(ValkyrieMirError::ValueTypeMismatch { function: *key, value: *value, expected: *ty });
                        }
                        continue;
                    }
                    if !block_defined.insert(*value) {
                        return Err(ValkyrieMirError::DuplicateBlockDefinition { function: *key, value: *value });
                    }
                    if function.value_types.get(value) != Some(ty) {
                        return Err(ValkyrieMirError::ValueTypeMismatch { function: *key, value: *value, expected: *ty });
                    }
                }
                for instruction in &block.instructions {
                    let uses = match &instruction.operation {
                        ValkyrieMirOperation::Invoke { callee, arguments } => {
                            match callee {
                                ValkyrieMirCallee::Item(callee) => {
                                    let Some(callee_record) = linked.item_instances.get(callee)
                                    else {
                                        return Err(ValkyrieMirError::UnknownCallee { function: *key, callee: *callee });
                                    };
                                    for ty in callee_record.parameter_types.iter().chain(std::iter::once(&callee_record.return_type)) {
                                        if !linked.types.contains_key(ty) {
                                            return Err(ValkyrieMirError::UnknownType { function: *key, ty: *ty });
                                        }
                                    }
                                    if arguments.len() != callee_record.parameter_types.len() {
                                        return Err(ValkyrieMirError::CallArityMismatch { function: *key, callee: *callee });
                                    }
                                    let mut bindings = BTreeMap::new();
                                    for (value, expected) in arguments.iter().zip(&callee_record.parameter_types) {
                                        let Some(actual) = function.value_types.get(value)
                                        else {
                                            return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *value });
                                        };
                                        // 泛型 callee 的声明签名带 binder 形参：调用点按结构化匹配代入
                                        // `(owner, ordinal) → 实参类型身份`，同一 binder 只允许一个绑定。
                                        if !unify_call_signature(linked, *actual, *expected, &mut bindings, true) {
                                            return Err(ValkyrieMirError::CallArgumentTypeMismatch {
                                                function: *key,
                                                callee: *callee,
                                                value: *value,
                                            });
                                        }
                                    }
                                    if let Some(result) = instruction.results.first() {
                                        let Some(actual) = function.value_types.get(result)
                                        else {
                                            // 结果 SSA 值由本指令自身定义，缺类型记录不是「定义前使用」：
                                            // canonical 无法证明结果类型与 callee 实例化返回类型一致，
                                            // 按调用结果合同失败关闭。
                                            return Err(ValkyrieMirError::CallResultTypeMismatch {
                                                function: *key,
                                                callee: *callee,
                                                value: *result,
                                            });
                                        };
                                        // callee 的返回类型可能只由自身 binder 参数化（参数里不出现，如
                                        // `SwissTable.new(capacity) -> SwissTable<K, V>`），因此结果类型也必须
                                        // 作为该返回签名的实例来匹配；参数阶段已建立的绑定仍必须一致。
                                        if !unify_call_signature(linked, *actual, callee_record.return_type, &mut bindings, true) {
                                            return Err(ValkyrieMirError::CallResultTypeMismatch {
                                                function: *key,
                                                callee: *callee,
                                                value: *result,
                                            });
                                        }
                                    }
                                    let returns_unit = matches!(
                                        linked.types.get(&callee_record.return_type).map(|row| &row.kind),
                                        Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Unit | CanonicalPrimitiveType::Void))
                                    );
                                    if instruction.results.len() > 1 || (!returns_unit && instruction.results.is_empty()) {
                                        return Err(ValkyrieMirError::OperationResultArityMismatch {
                                            function: *key,
                                            instruction: instruction.id,
                                        });
                                    }
                                }
                                ValkyrieMirCallee::Witness(witness) => {
                                    // 具体 witness 必须在实例化固定点消去为 `Item`；走到
                                    // canonical 校验说明上游未闭合，按未消去分派失败关闭。
                                    // 目标层不得再扫描实例库补做这一步。
                                    return Err(ValkyrieMirError::UnresolvedWitnessDispatch {
                                        trait_declaration: witness.trait_declaration,
                                        method_declaration: witness.method_declaration,
                                        self_type: witness.self_type,
                                        candidates: 0,
                                    });
                                }
                                ValkyrieMirCallee::Value(callee) => {
                                    let Some(callee_type) = function.value_types.get(callee)
                                    else {
                                        return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *callee });
                                    };
                                    let Some(CanonicalTypeKind::Function { parameters, return_type }) =
                                        linked.types.get(callee_type).map(|record| &record.kind)
                                    else {
                                        return Err(ValkyrieMirError::ValueCalleeNotFunction { function: *key, value: *callee });
                                    };
                                    if arguments.len() != parameters.len() {
                                        return Err(ValkyrieMirError::ValueCalleeArityMismatch { function: *key, value: *callee });
                                    }
                                    for (argument, expected) in arguments.iter().zip(parameters) {
                                        if function.value_types.get(argument) != Some(expected) {
                                            return Err(ValkyrieMirError::ValueCalleeArgumentTypeMismatch {
                                                function: *key,
                                                value: *callee,
                                                argument: *argument,
                                            });
                                        }
                                    }
                                    if let Some(result) = instruction.results.first() {
                                        if function.value_types.get(result) != Some(return_type) {
                                            return Err(ValkyrieMirError::ValueCalleeResultTypeMismatch {
                                                function: *key,
                                                value: *callee,
                                                result: *result,
                                            });
                                        }
                                    }
                                    let returns_unit = matches!(
                                        linked.types.get(return_type).map(|row| &row.kind),
                                        Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Unit | CanonicalPrimitiveType::Void))
                                    );
                                    if instruction.results.len() > 1 || (!returns_unit && instruction.results.is_empty()) {
                                        return Err(ValkyrieMirError::OperationResultArityMismatch {
                                            function: *key,
                                            instruction: instruction.id,
                                        });
                                    }
                                }
                            }
                            let mut uses = arguments.clone();
                            if let ValkyrieMirCallee::Value(callee) = callee {
                                uses.push(*callee);
                            }
                            uses
                        }
                        ValkyrieMirOperation::Copy { source } => {
                            if instruction.results.len() != 1
                                || instruction.results.first().and_then(|value| function.value_types.get(value))
                                    != function.value_types.get(source)
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*source]
                        }
                        ValkyrieMirOperation::Cast { source, ty } => {
                            if !linked.types.contains_key(ty) {
                                return Err(ValkyrieMirError::UnknownType { function: *key, ty: *ty });
                            }
                            if function.value_types.get(source).is_none() {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *source,
                                });
                            }
                            if instruction.results.len() != 1
                                || instruction.results.first().and_then(|value| function.value_types.get(value)) != Some(ty)
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*source]
                        }
                        ValkyrieMirOperation::StoreVar { name: _, value, ty } => {
                            if instruction.results.len() > 1 {
                                return Err(ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id });
                            }
                            if let Some(result) = instruction.results.first() {
                                let same_type = match (function.value_types.get(result), function.value_types.get(value)) {
                                    (Some(result_ty), Some(value_ty)) => {
                                        result_ty == value_ty || canonical_text_encoding_compatible(&linked, *result_ty, *value_ty)
                                    }
                                    _ => false,
                                };
                                if !same_type {
                                    return Err(ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id });
                                }
                            }
                            if let Some(expected_ty) = ty {
                                if !linked.types.contains_key(expected_ty) {
                                    return Err(ValkyrieMirError::UnknownType { function: *key, ty: *expected_ty });
                                }
                                if function.value_types.get(value).is_some_and(|actual| !matches_declared_type(&linked, *actual, *expected_ty))
                                {
                                    return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                        function: *key,
                                        instruction: instruction.id,
                                        value: *value,
                                    });
                                }
                            }
                            vec![*value]
                        }
                        ValkyrieMirOperation::AggregateCopy { source, destination } => {
                            let source_type = function.value_types.get(source);
                            let destination_type = function.value_types.get(destination);
                            if source_type.is_none() || source_type != destination_type {
                                return Err(ValkyrieMirError::ValueTypeMismatch {
                                    function: *key,
                                    value: *destination,
                                    expected: source_type.copied().unwrap_or(function.return_type),
                                });
                            }
                            if instruction.results.as_slice() != [*destination] {
                                return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *destination });
                            }
                            vec![*source]
                        }
                        ValkyrieMirOperation::LoadConstant { constant } => {
                            if instruction.results.len() != 1 {
                                return Err(ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id });
                            }
                            // 空值必须承载 `T?` 上下文类型；没有 nullable 结果类型时
                            // 不得静默降级成 Unit 或名义 Option。
                            if matches!(constant, ValkyrieMirConstant::Null) {
                                let result_is_nullable = instruction
                                    .results
                                    .first()
                                    .and_then(|value| function.value_types.get(value))
                                    .and_then(|ty| linked.types.get(ty))
                                    .is_some_and(|row| matches!(row.kind, CanonicalTypeKind::Nullable(_)));
                                if !result_is_nullable {
                                    return Err(ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id });
                                }
                            }
                            Vec::new()
                        }
                        ValkyrieMirOperation::LoadFunctionRef { instance, environment, .. } => {
                            // 函数作值只携带冻结实例身份：必须存在于已链接闭包，
                            // 结果必须恰好一个且为 Function 类型 kind；否则失败关闭。
                            if !linked.item_instances.contains_key(instance) {
                                return Err(ValkyrieMirError::UnknownCallee { function: *key, callee: *instance });
                            }
                            if instruction.results.len() != 1
                                || !instruction.results.first().and_then(|value| function.value_types.get(value)).is_some_and(|ty| {
                                    matches!(linked.types.get(ty).map(|row| &row.kind), Some(CanonicalTypeKind::Function { .. }))
                                })
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            // 捕获环境是普通 SSA 聚合值，参与读集合；常量环境无 SSA 身份，
                            // 视为操作数合同失败，不允许目标侧静默重造。
                            match environment {
                                Some(ValkyrieMirOperand::Value(value)) => vec![*value],
                                Some(ValkyrieMirOperand::Constant(_)) => {
                                    return Err(ValkyrieMirError::FunctionEnvironmentNotValue {
                                        function: *key,
                                        instruction: instruction.id,
                                    });
                                }
                                None => Vec::new(),
                            }
                        }
                        ValkyrieMirOperation::SumNew { nominal, variant, payload } => {
                            let Some(record) = linked.variants.get(&(*nominal, *variant))
                            else {
                                return Err(ValkyrieMirError::UnknownVariant { function: *key, variant: *variant });
                            };
                            let Some(owner) = linked.nominal_instances.get(nominal)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: *nominal });
                            };
                            let result_type = instruction.results.first().and_then(|value| function.value_types.get(value));
                            let result_is_owner = result_type.is_some_and(|ty| canonical_nominal_owner_matches(&linked, *ty, owner.ty));
                            if instruction.results.len() != 1 || !result_is_owner {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            match (record.payload_type, payload) {
                                // 文本载荷允许「内建编码基元 ↔ 登记载体名义类型」：两者是同一值空间
                                // 的不同形态（`type utf8 = Utf8Text`），与调用实参合同同口径。
                                (Some(expected), Some(value))
                                    if function
                                        .value_types
                                        .get(value)
                                        .is_some_and(|actual| *actual == expected || canonical_text_encoding_compatible(linked, *actual, expected)) =>
                                {
                                    vec![*value]
                                }
                                (None, None) => Vec::new(),
                                _ => {
                                    return Err(ValkyrieMirError::VariantPayloadMismatch { function: *key, variant: *variant });
                                }
                            }
                        }
                        ValkyrieMirOperation::SumPayloadGet { nominal, variant, object } => {
                            let Some(record) = linked.variants.get(&(*nominal, *variant))
                            else {
                                return Err(ValkyrieMirError::UnknownVariant { function: *key, variant: *variant });
                            };
                            if record.payload_type.is_none() {
                                return Err(ValkyrieMirError::VariantPayloadMismatch { function: *key, variant: *variant });
                            }
                            if instruction.results.len() != 1
                                || !instruction.results.first().and_then(|value| function.value_types.get(value)).is_some_and(|actual| {
                                    record.payload_type.as_ref().is_some_and(|expected| {
                                        actual == expected || canonical_text_encoding_compatible(linked, *actual, *expected)
                                    })
                                })
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            let Some(object_type) = function.value_types.get(object)
                            else {
                                return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *object });
                            };
                            let Some(owner) = linked.nominal_instances.get(nominal)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: *nominal });
                            };
                            if !canonical_nominal_owner_matches(&linked, *object_type, owner.ty) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *object,
                                });
                            }
                            vec![*object]
                        }
                        ValkyrieMirOperation::SumVariantIs { nominal, variant, object } => {
                            let Some(_record) = linked.variants.get(&(*nominal, *variant))
                            else {
                                return Err(ValkyrieMirError::UnknownVariant { function: *key, variant: *variant });
                            };
                            let bool_result = instruction.results.len() == 1
                                && instruction.results.first().and_then(|value| function.value_types.get(value)).is_some_and(|ty| {
                                    matches!(
                                        linked.types.get(ty).map(|row| &row.kind),
                                        Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Bool))
                                    )
                                });
                            if !bool_result {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            let Some(object_type) = function.value_types.get(object)
                            else {
                                return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *object });
                            };
                            let Some(owner) = linked.nominal_instances.get(nominal)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: *nominal });
                            };
                            if !canonical_nominal_owner_matches(&linked, *object_type, owner.ty) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *object,
                                });
                            }
                            vec![*object]
                        }
                        ValkyrieMirOperation::StructNew { nominal, fields } => {
                            let Some(nominal_record) = linked.nominal_instances.get(nominal)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: *nominal });
                            };
                            let mut seen = std::collections::BTreeSet::new();
                            for (field, value) in fields {
                                if !seen.insert(*field) {
                                    return Err(ValkyrieMirError::DuplicateStructField { function: *key, nominal: *nominal, field: *field });
                                }
                                let Some(record) = linked.fields.get(field)
                                else {
                                    return Err(ValkyrieMirError::UnknownField { function: *key, field: *field });
                                };
                                if record.owner != *nominal {
                                    return Err(ValkyrieMirError::FieldOwnerMismatch { function: *key, field: *field, nominal: *nominal });
                                }
                                if !function.value_types.get(value).is_some_and(|actual| matches_declared_type(linked, *actual, record.ty)) {
                                    return Err(ValkyrieMirError::FieldTypeMismatch { function: *key, field: *field, value: *value });
                                }
                            }
                            for field in &nominal_record.fields {
                                if !seen.contains(field) {
                                    return Err(ValkyrieMirError::MissingStructField { function: *key, nominal: *nominal, field: *field });
                                }
                            }
                            // 结果可以是同一名义声明的参数化实例（开放泛型函数体里构造 `ArrayList<T>`）；
                            // owner 关系按声明身份判定，与 FieldGet/FieldSet 同口径。
                            let result_matches_owner = instruction.results.len() == 1
                                && instruction
                                    .results
                                    .first()
                                    .and_then(|value| function.value_types.get(value))
                                    .is_some_and(|ty| canonical_nominal_owner_matches(&linked, *ty, nominal_record.ty));
                            if !result_matches_owner {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            fields.iter().map(|(_, value)| *value).collect()
                        }
                        ValkyrieMirOperation::FieldGet { object, field } => {
                            let Some(record) = linked.fields.get(field)
                            else {
                                return Err(ValkyrieMirError::UnknownField { function: *key, field: *field });
                            };
                            let Some(object_ty) = function.value_types.get(object)
                            else {
                                return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *object });
                            };
                            let Some(owner) = linked.nominal_instances.get(&record.owner)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: record.owner });
                            };
                            if !canonical_nominal_owner_matches(&linked, *object_ty, owner.ty) {
                                return Err(ValkyrieMirError::FieldOwnerMismatch { function: *key, field: *field, nominal: record.owner });
                            }
                            let field_result_matches = instruction
                                .results
                                .first()
                                .and_then(|value| function.value_types.get(value))
                                .is_some_and(|actual| matches_declared_type(&linked, *actual, record.ty));
                            if !field_result_matches {
                                return Err(ValkyrieMirError::FieldTypeMismatch {
                                    function: *key,
                                    field: *field,
                                    value: instruction.results.first().copied().unwrap_or(*object),
                                });
                            }
                            vec![*object]
                        }
                        ValkyrieMirOperation::FieldSet { object, field, value } => {
                            if !instruction.results.is_empty() {
                                return Err(ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id });
                            }
                            let Some(record) = linked.fields.get(field)
                            else {
                                return Err(ValkyrieMirError::UnknownField { function: *key, field: *field });
                            };
                            let Some(object_ty) = function.value_types.get(object)
                            else {
                                return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value: *object });
                            };
                            let Some(owner) = linked.nominal_instances.get(&record.owner)
                            else {
                                return Err(ValkyrieMirError::UnknownNominal { function: *key, nominal: record.owner });
                            };
                            if !canonical_nominal_owner_matches(&linked, *object_ty, owner.ty) {
                                return Err(ValkyrieMirError::FieldOwnerMismatch { function: *key, field: *field, nominal: record.owner });
                            }
                            if !function.value_types.get(value).is_some_and(|actual| matches_declared_type(&linked, *actual, record.ty)) {
                                return Err(ValkyrieMirError::FieldTypeMismatch { function: *key, field: *field, value: *value });
                            }
                            vec![*object, *value]
                        }
                        ValkyrieMirOperation::ArrayGet { array, index } => {
                            let Some(CanonicalTypeKind::Array { element, .. }) =
                                function.value_types.get(array).and_then(|ty| linked.types.get(ty)).map(|row| &row.kind)
                            else {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *array,
                                });
                            };
                            // 整数形状由前端冻结进 `array_index_types`（原生 `Integer` 或 `usize` 等名义整数）；
                            // canonical 只认已登记的 TypeId，不按名字重判。
                            let index_is_integer = function.value_types.get(index).is_some_and(|ty| linked.is_array_index_type(*ty));
                            if !index_is_integer {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *index,
                                });
                            }
                            if instruction.results.len() != 1
                                || !instruction
                                    .results
                                    .first()
                                    .and_then(|value| function.value_types.get(value))
                                    .is_some_and(|actual| matches_declared_type(linked, *actual, *element))
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*array, *index]
                        }
                        ValkyrieMirOperation::ArrayNew { array_type, length, initialization } => {
                            let Some(CanonicalTypeKind::Array { element, .. }) = linked.types.get(array_type).map(|row| &row.kind)
                            else {
                                return Err(ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id });
                            };
                            let length_is_integer = function.value_types.get(length).is_some_and(|ty| linked.is_array_index_type(*ty));
                            let fill_is_valid = match initialization {
                                ValkyrieMirArrayInitialization::Default => true,
                                ValkyrieMirArrayInitialization::Fill(value) => {
                                    function.value_types.get(value).is_some_and(|actual| matches_declared_type(linked, *actual, *element))
                                }
                            };
                            if !length_is_integer || !fill_is_valid {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *length,
                                });
                            }
                            if instruction.results.len() != 1
                                || instruction.results.first().and_then(|value| function.value_types.get(value)) != Some(array_type)
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            match initialization {
                                ValkyrieMirArrayInitialization::Default => vec![*length],
                                ValkyrieMirArrayInitialization::Fill(value) => vec![*length, *value],
                            }
                        }
                        ValkyrieMirOperation::ArrayFromElements { array_type, elements } => {
                            let Some(CanonicalTypeKind::Array { element, .. }) = linked.types.get(array_type).map(|row| &row.kind)
                            else {
                                return Err(ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id });
                            };
                            if instruction.results.len() != 1
                                || instruction.results.first().and_then(|value| function.value_types.get(value)) != Some(array_type)
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            if elements.iter().any(|value| {
                                function.value_types.get(value).is_some_and(|actual| !matches_declared_type(linked, *actual, *element))
                            }) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *elements.iter().find(|value| function.value_types.get(value).is_some_and(|actual| !matches_declared_type(linked, *actual, *element))).unwrap(),
                                });
                            }
                            elements.clone()
                        }
                        ValkyrieMirOperation::ArraySet { array, index, value } => {
                            let Some(CanonicalTypeKind::Array { element, .. }) =
                                function.value_types.get(array).and_then(|ty| linked.types.get(ty)).map(|row| &row.kind)
                            else {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *array,
                                });
                            };
                            let index_is_integer = function.value_types.get(index).is_some_and(|ty| linked.is_array_index_type(*ty));
                            if !index_is_integer
                                || !function.value_types.get(value).is_some_and(|actual| matches_declared_type(linked, *actual, *element))
                            {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: if !index_is_integer { *index } else { *value },
                                });
                            }
                            if !instruction.results.is_empty() {
                                return Err(ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id });
                            }
                            vec![*array, *index, *value]
                        }
                        ValkyrieMirOperation::ArrayLength { array } => {
                            if !matches!(
                                function.value_types.get(array).and_then(|ty| linked.types.get(ty)).map(|row| &row.kind),
                                Some(CanonicalTypeKind::Array { .. })
                            ) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *array,
                                });
                            }
                            let result_is_integer = instruction.results.len() == 1
                                && instruction
                                    .results
                                    .first()
                                    .and_then(|value| function.value_types.get(value))
                                    .is_some_and(|ty| linked.is_array_index_type(*ty));
                            if !result_is_integer {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*array]
                        }
                        ValkyrieMirOperation::NullIsNull { value: operand } => {
                            if !matches!(
                                function.value_types.get(operand).and_then(|ty| linked.types.get(ty)).map(|row| &row.kind),
                                Some(CanonicalTypeKind::Nullable(_))
                            ) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *operand,
                                });
                            }
                            let result_is_boolean = instruction.results.len() == 1
                                && instruction
                                    .results
                                    .first()
                                    .and_then(|value| function.value_types.get(value))
                                    .and_then(|ty| linked.types.get(ty))
                                    .is_some_and(|row| matches!(row.kind, CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Bool)));
                            if !result_is_boolean {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*operand]
                        }
                        ValkyrieMirOperation::NullUnwrap { value: operand } => {
                            let Some(CanonicalTypeKind::Nullable(payload)) =
                                function.value_types.get(operand).and_then(|ty| linked.types.get(ty)).map(|row| &row.kind)
                            else {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *operand,
                                });
                            };
                            if instruction.results.len() != 1
                                || instruction.results.first().and_then(|value| function.value_types.get(value)) != Some(payload)
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            vec![*operand]
                        }
                        ValkyrieMirOperation::TupleNew { element_types, fields } => {
                            let Some(result) = instruction.results.first().and_then(|value| function.value_types.get(value))
                            else {
                                return Err(ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id });
                            };
                            // 标签属于类型身份，`TupleNew` 只按位置构造：结果允许
                            // `LabeledTuple`，但元素类型必须与 `element_types` 逐一相等。
                            let kind_matches = match linked.types.get(result).map(|row| &row.kind) {
                                Some(CanonicalTypeKind::Tuple(members)) => members == element_types,
                                Some(CanonicalTypeKind::LabeledTuple { fields }) => {
                                    fields.iter().map(|(_, ty)| ty).eq(element_types.iter())
                                }
                                _ => false,
                            };
                            if instruction.results.len() != 1 || !kind_matches
                            {
                                return Err(if instruction.results.len() == 1 {
                                    ValkyrieMirError::OperationResultTypeMismatch { function: *key, instruction: instruction.id }
                                }
                                else {
                                    ValkyrieMirError::OperationResultArityMismatch { function: *key, instruction: instruction.id }
                                });
                            }
                            if fields.len() != element_types.len() {
                                return Err(ValkyrieMirError::OperationOperandArityMismatch { function: *key, instruction: instruction.id });
                            }
                            if let Some((value, _)) =
                                fields.iter().zip(element_types).find(|(value, expected)| function.value_types.get(value) != Some(expected))
                            {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *value,
                                });
                            }
                            fields.clone()
                        }
                        ValkyrieMirOperation::TupleGet { object, tuple_type, index } => {
                            let arity = match linked.types.get(tuple_type).map(|row| &row.kind) {
                                Some(CanonicalTypeKind::Tuple(members)) => members.len(),
                                Some(CanonicalTypeKind::LabeledTuple { fields }) => fields.len(),
                                _ => {
                                    return Err(ValkyrieMirError::OperationResultTypeMismatch {
                                        function: *key,
                                        instruction: instruction.id,
                                    });
                                }
                            };
                            if (*index as usize) >= arity {
                                return Err(ValkyrieMirError::OperationOperandArityMismatch { function: *key, instruction: instruction.id });
                            }
                            if function.value_types.get(object) != Some(tuple_type) {
                                return Err(ValkyrieMirError::OperationOperandTypeMismatch {
                                    function: *key,
                                    instruction: instruction.id,
                                    value: *object,
                                });
                            }
                            vec![*object]
                        }
                    };
                    if uses.iter().any(|value| !value_is_available(value)) {
                        let value = *uses.iter().find(|value| !value_is_available(value)).unwrap();
                        return Err(ValkyrieMirError::UseBeforeDefinition { function: *key, value });
                    }
                    for value in &uses {
                        if let Some(Some(definition_block)) = definitions.get(value) {
                            if definition_block != block_id && !dominators[block_id].contains(definition_block) {
                                return Err(ValkyrieMirError::NonDominatingUse { function: *key, value: *value, block: *block_id });
                            }
                        }
                    }
                    for result in &instruction.results {
                        if !block_defined.insert(*result) {
                            return Err(ValkyrieMirError::DuplicateDefinition { function: *key, value: *result });
                        }
                        if !function.value_types.contains_key(result) {
                            return Err(ValkyrieMirError::ValueTypeMismatch {
                                function: *key,
                                value: *result,
                                expected: function.return_type,
                            });
                        }
                    }
                }
                let (targets, terminator_values): (Vec<_>, Vec<_>) = match &block.terminator {
                    ValkyrieMirTerminator::Return { value } => {
                        if let Some(value) = value {
                            if !function
                                .value_types
                                .get(value)
                                .is_some_and(|actual| canonical_types_compatible(&linked, *actual, function.return_type))
                            {
                                return Err(ValkyrieMirError::ReturnTypeMismatch { function: *key, value: *value });
                            }
                        }
                        (Vec::new(), value.iter().copied().collect())
                    }
                    ValkyrieMirTerminator::Jump { target, arguments } => (vec![(*target, arguments.len())], arguments.clone()),
                    ValkyrieMirTerminator::Branch { condition, then_target, else_target } => {
                        (vec![(*then_target, 0), (*else_target, 0)], vec![*condition])
                    }
                    ValkyrieMirTerminator::PerformEffect { effect, resume_target, payload } => {
                        let payload = payload.ok_or(ValkyrieMirError::MissingEffectPayload { function: *key, block: *block_id })?;
                        let resume_arity = usize::from(*effect != ValkyrieMirEffectKind::AsyncSpawn);
                        (vec![(*resume_target, resume_arity)], vec![payload])
                    }
                    ValkyrieMirTerminator::StateDispatch { state, cases, default_target } => {
                        let state_type = function.value_types.get(state).and_then(|ty| linked.types.get(ty));
                        if !matches!(
                            state_type.map(|record| &record.kind),
                            Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { .. }))
                        ) {
                            return Err(ValkyrieMirError::InvalidDispatchState { function: *key, value: *state });
                        }
                        let mut states = std::collections::BTreeSet::new();
                        for (state, _) in cases {
                            if !states.insert(*state) {
                                return Err(ValkyrieMirError::DuplicateDispatchState { function: *key, block: *block_id, state: *state });
                            }
                        }
                        (cases.iter().map(|(_, target)| (*target, 0)).chain(std::iter::once((*default_target, 0))).collect(), vec![*state])
                    }
                    ValkyrieMirTerminator::YieldToRuntime { payload, .. } => {
                        let payload = payload.ok_or(ValkyrieMirError::MissingEffectPayload { function: *key, block: *block_id })?;
                        (Vec::new(), vec![payload])
                    }
                    ValkyrieMirTerminator::Unreachable => (Vec::new(), Vec::new()),
                };
                for value in terminator_values {
                    if !value_is_available(&value) {
                        return Err(ValkyrieMirError::TerminatorUseBeforeDefinition { function: *key, value });
                    }
                    if let Some(Some(definition_block)) = definitions.get(&value) {
                        if definition_block != block_id && !dominators[block_id].contains(definition_block) {
                            return Err(ValkyrieMirError::NonDominatingUse { function: *key, value, block: *block_id });
                        }
                    }
                }
                for (target, arity) in targets {
                    let Some(target_block) = function.blocks.get(&target)
                    else {
                        return Err(ValkyrieMirError::UnknownBlock { function: *key, block: target });
                    };
                    if target_block.parameters.len() != arity {
                        return Err(ValkyrieMirError::BlockParameterArityMismatch { function: *key, block: target });
                    }
                    if let ValkyrieMirTerminator::Jump { arguments, .. } = &block.terminator {
                        for (value, (_, expected)) in arguments.iter().zip(&target_block.parameters) {
                            // 文本 carrier 的内建编码与名义声明是同一值空间：与本校验器其余站点
                            // （调用签名、`StoreVar`、`Return`）共用 `canonical_text_encoding_compatible`
                            // 事实，SSA 合并块的块参数与入边实参不得因两种文本表示判为类型不符。
                            let actual = function.value_types.get(value);
                            if actual != Some(expected) && !actual.is_some_and(|actual| canonical_text_encoding_compatible(linked, *actual, *expected)) {
                                return Err(ValkyrieMirError::BlockParameterTypeMismatch { function: *key, block: target, value: *value });
                            }
                        }
                    }
                }
                let _ = block_id;
            }
        }
        Ok(())
    }
}

/// `actual` 是否为声明侧类型 `declared` 的实例。
///
/// 声明侧类型可以带 binder 形参（owner + ordinal）：字段、元素等声明类型与
/// 开放泛型实例的实际类型按结构逐层匹配，形参叶子绑定到实际子类型。
fn matches_declared_type(linked: &LinkedSemanticProgram, actual: TypeId, declared: TypeId) -> bool {
    actual == declared || unify_call_signature(linked, actual, declared, &mut BTreeMap::new(), true)
}

/// 调用点的泛型签名实例化。
///
/// `expected` 是 callee 已冻结的声明签名；其中带 binder 的形参叶子由 binder 身份
/// `(owner, ordinal)` 表示，不是名字。调用点把每个首次出现的 binder 绑定到
/// `actual` 的对应子类型身份，同一 binder 在同一调用内必须绑定同一类型；
/// 结构层（Nominal/Tuple/Array/Nullable/Union/Intersection/Function）必须逐层一致。
/// `allow_new_bindings = false` 用于返回类型：只能复用参数阶段已建立的绑定。
fn unify_call_signature(
    linked: &LinkedSemanticProgram,
    actual: TypeId,
    expected: TypeId,
    bindings: &mut BTreeMap<(ItemId, u32), TypeId>,
    allow_new_bindings: bool,
) -> bool {
    if actual == expected || canonical_text_encoding_compatible(linked, actual, expected) {
        return true;
    }
    let Some(expected_kind) = linked.types.get(&expected).map(|row| &row.kind)
    else {
        return false;
    };
    if let CanonicalTypeKind::Generic { owner, ordinal } = expected_kind {
        return match bindings.get(&(*owner, *ordinal)) {
            Some(bound) => unify_call_signature(linked, actual, *bound, bindings, false),
            None if allow_new_bindings => {
                bindings.insert((*owner, *ordinal), actual);
                true
            }
            None => false,
        };
    }
    let Some(actual_kind) = linked.types.get(&actual).map(|row| &row.kind)
    else {
        return false;
    };
    match (actual_kind, expected_kind) {
        (
            CanonicalTypeKind::Nominal { declaration: actual_declaration, arguments: actual_arguments },
            CanonicalTypeKind::Nominal { declaration: expected_declaration, arguments: expected_arguments },
        ) => {
            actual_declaration == expected_declaration
                && actual_arguments.len() == expected_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, expected)| unify_call_signature(linked, *actual, *expected, bindings, allow_new_bindings))
        }
        (CanonicalTypeKind::Tuple(actual_items), CanonicalTypeKind::Tuple(expected_items))
        | (CanonicalTypeKind::Union(actual_items), CanonicalTypeKind::Union(expected_items))
        | (CanonicalTypeKind::Intersection(actual_items), CanonicalTypeKind::Intersection(expected_items)) => {
            actual_items.len() == expected_items.len()
                && actual_items
                    .iter()
                    .zip(expected_items)
                    .all(|(actual, expected)| unify_call_signature(linked, *actual, *expected, bindings, allow_new_bindings))
        }
        // 带标签元组的标签也是身份的一部分；与无标签元组不做隐式统一。
        (CanonicalTypeKind::LabeledTuple { fields: actual_items }, CanonicalTypeKind::LabeledTuple { fields: expected_items })
            if actual_items.len() == expected_items.len() =>
        {
            actual_items.iter().zip(expected_items).all(|((actual_name, actual), (expected_name, expected))| {
                actual_name == expected_name && unify_call_signature(linked, *actual, *expected, bindings, allow_new_bindings)
            })
        }
        (
            CanonicalTypeKind::Array { element: actual_element, length: actual_length },
            CanonicalTypeKind::Array { element: expected_element, length: expected_length },
        ) => actual_length == expected_length && unify_call_signature(linked, *actual_element, *expected_element, bindings, allow_new_bindings),
        (CanonicalTypeKind::Nullable(actual_element), CanonicalTypeKind::Nullable(expected_element)) => {
            unify_call_signature(linked, *actual_element, *expected_element, bindings, allow_new_bindings)
        }
        (
            CanonicalTypeKind::Function { parameters: actual_parameters, return_type: actual_return },
            CanonicalTypeKind::Function { parameters: expected_parameters, return_type: expected_return },
        ) => {
            actual_parameters.len() == expected_parameters.len()
                && actual_parameters
                    .iter()
                    .zip(expected_parameters)
                    .all(|(actual, expected)| unify_call_signature(linked, *actual, *expected, bindings, allow_new_bindings))
                && unify_call_signature(linked, *actual_return, *expected_return, bindings, allow_new_bindings)
        }
        _ => false,
    }
}
// ---------------------------------------------------------------------------
// 链接事实验证（Valkyrie 语言合同的一部分，不归 Nyar 公共类型）
// ---------------------------------------------------------------------------

/// 判断一个具体类型是否属于给定的名义实例。
///
/// `NominalInstanceRecord::ty` 保存声明形状，而调用点可能携带已经代入的
/// 泛型参数。两者的参数列表不应被当成不同的 owner；名义 declaration
/// identity 才是 ADT/字段/variant 所属关系的事实来源。
pub(crate) fn canonical_nominal_owner_matches(linked: &LinkedSemanticProgram, actual: TypeId, owner: TypeId) -> bool {
    if actual == owner {
        return true;
    }
    let Some(actual_kind) = linked.types.get(&actual).map(|record| &record.kind)
    else {
        return false;
    };
    let Some(owner_kind) = linked.types.get(&owner).map(|record| &record.kind)
    else {
        return false;
    };
    match (actual_kind, owner_kind) {
        (
            CanonicalTypeKind::Nominal { declaration: actual_declaration, arguments: actual_arguments },
            CanonicalTypeKind::Nominal { declaration: owner_declaration, arguments: owner_arguments },
        ) => {
            actual_declaration == owner_declaration
                && (actual_arguments == owner_arguments || actual_arguments.is_empty() || owner_arguments.is_empty())
        }
        _ => false,
    }
}

pub(crate) fn canonical_types_compatible(linked: &LinkedSemanticProgram, actual: TypeId, expected: TypeId) -> bool {
    if actual == expected || canonical_text_encoding_compatible(linked, actual, expected) {
        return true;
    }
    let (Some(actual_kind), Some(expected_kind)) =
        (linked.types.get(&actual).map(|record| &record.kind), linked.types.get(&expected).map(|record| &record.kind))
    else {
        return false;
    };
    match (actual_kind, expected_kind) {
        (
            CanonicalTypeKind::Nominal { declaration: actual_declaration, arguments: actual_arguments },
            CanonicalTypeKind::Nominal { declaration: expected_declaration, arguments: expected_arguments },
        ) => {
            actual_declaration == expected_declaration
                && actual_arguments.len() == expected_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, expected)| canonical_types_compatible(linked, *actual, *expected))
        }
        (
            CanonicalTypeKind::Array { element: actual, length: actual_length },
            CanonicalTypeKind::Array { element: expected, length: expected_length },
        ) => actual_length == expected_length && canonical_types_compatible(linked, *actual, *expected),
        (CanonicalTypeKind::Nullable(actual), CanonicalTypeKind::Nullable(expected)) => canonical_types_compatible(linked, *actual, *expected),
        // `T` 可隐式提升为 `T?`：非空值就是同一引用的 Some 侧，不是名义 Option 变体。
        (_, CanonicalTypeKind::Nullable(inner)) => canonical_types_compatible(linked, actual, *inner),
        (CanonicalTypeKind::Tuple(actual), CanonicalTypeKind::Tuple(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| canonical_types_compatible(linked, *actual, *expected))
        }
        // 标签是类型身份的一部分：同名同型同长度才兼容；带标签 ↔ 无标签没有隐式兼容。
        (CanonicalTypeKind::LabeledTuple { fields: actual }, CanonicalTypeKind::LabeledTuple { fields: expected })
            if actual.len() == expected.len() =>
        {
            actual.iter().zip(expected).all(|((actual_name, actual_ty), (expected_name, expected_ty))| {
                actual_name == expected_name && canonical_types_compatible(linked, *actual_ty, *expected_ty)
            })
        }
        // 形参身份由 `binder`(owner, ordinal) 决定：同一 binder 才是同一形参。
        (
            CanonicalTypeKind::Generic { owner: actual_owner, ordinal: actual_ordinal },
            CanonicalTypeKind::Generic { owner: expected_owner, ordinal: expected_ordinal },
        ) => actual_owner == expected_owner && actual_ordinal == expected_ordinal,
        _ => false,
    }
}

/// 文本编码基元与文本载体名义类型之间是否等价。
pub(crate) fn canonical_text_encoding_compatible(linked: &LinkedSemanticProgram, left: TypeId, right: TypeId) -> bool {
    fn encoding(linked: &LinkedSemanticProgram, ty: TypeId) -> Option<CanonicalPrimitiveType> {
        match linked.types.get(&ty).map(|record| &record.kind) {
            Some(CanonicalTypeKind::Primitive(primitive @ (CanonicalPrimitiveType::Utf8 | CanonicalPrimitiveType::Utf16))) => {
                Some(*primitive)
            }
            _ => linked.text_encoding_types.get(&ty).copied(),
        }
    }
    match (encoding(linked, left), encoding(linked, right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

/// 拒绝「已有冻结布局或载体身份，却仍以 `Named` 短名残留」的类型。
///
/// 命中即结构化失败，并点名阶段、产生函数与 owner，不降级为告警。
pub fn validate_named_residues(linked: &LinkedSemanticProgram) -> Result<(), ValkyrieMirError> {
    for layout in &linked.aggregate_layouts.layouts {
        for field in &layout.fields {
            if let Some(name) = shadowed_named_residue(linked, &field.ty) {
                return Err(ValkyrieMirError::ShadowedNamedResidue {
                    stage: "canonical::aggregate_layouts".into(),
                    function: "value_semantics::layout_for_struct / value_semantics::instance_layout_for_type".into(),
                    owner: format!("{}::{}.{}", layout.namespace, layout.name, field.name),
                    name: name.as_str().into(),
                });
            }
        }
    }
    for sum in &linked.sum_types {
        for variant in &sum.variants {
            if let Some(name) = variant.payload_type.as_ref().and_then(|payload| shadowed_named_residue(linked, payload)) {
                return Err(ValkyrieMirError::ShadowedNamedResidue {
                    stage: "canonical::sum_layouts".into(),
                    function: "canonical::identities::sum_payload_layout / canonical::identities::record_payload_layout".into(),
                    owner: format!("{}.{}", sum.name, variant.name),
                    name: name.as_str().into(),
                });
            }
        }
    }
    Ok(())
}

/// 递归返回第一个与已登记身份同名的 `Named` 残留。
fn shadowed_named_residue<'a>(linked: &LinkedSemanticProgram, ty: &'a NyarType) -> Option<&'a Identifier> {
    match ty {
        NyarType::Named(name) if shadows_registered_identity(linked, name) => Some(name),
        NyarType::Named(_) => None,
        // `Apply` 的基是泛型构造器拼写（声明名），不是值位类型；身份由参数与整体
        // 应用承载。只检查实参，避免把合法的 `Apply(Named("Option"), …)` 误判为残留。
        NyarType::Apply(_, arguments) => arguments.iter().find_map(|argument| shadowed_named_residue(linked, argument)),
        NyarType::Function(function) => function
            .params
            .iter()
            .find_map(|parameter| shadowed_named_residue(linked, parameter))
            .or_else(|| shadowed_named_residue(linked, &function.return_type)),
        NyarType::Tuple(elements) | NyarType::Union(elements) => elements.iter().find_map(|element| shadowed_named_residue(linked, element)),
        NyarType::Array(element) | NyarType::Nullable(element) => shadowed_named_residue(linked, element),
        NyarType::FixedArray { element, .. } => shadowed_named_residue(linked, element),
        NyarType::TraitObject(object) => object.type_arguments.iter().find_map(|argument| shadowed_named_residue(linked, argument)),
        NyarType::Bottom
        | NyarType::Unit
        | NyarType::Boolean
        | NyarType::Integer8 { .. }
        | NyarType::Integer16 { .. }
        | NyarType::Integer32 { .. }
        | NyarType::Integer64 { .. }
        | NyarType::Integer128 { .. }
        | NyarType::Float32
        | NyarType::Float64
        | NyarType::Character
        | NyarType::Utf8
        | NyarType::Utf16
        | NyarType::Nominal { .. } => None,
    }
}

/// 该拼写是否属于已登记身份（布局 / 文本载体 / 原生标量）。
fn shadows_registered_identity(linked: &LinkedSemanticProgram, name: &Identifier) -> bool {
    let spelling = name.as_str();
    let short = spelling.rsplit("::").next().unwrap_or(spelling);
    linked.types.iter().any(|(id, record)| {
        let registered = linked.aggregate_layout_by_type.contains_key(id)
            || linked.text_encoding_types.contains_key(id)
            || matches!(record.kind, CanonicalTypeKind::Primitive(_));
        let declaration_identity = match &record.kind {
            CanonicalTypeKind::Nominal { arguments, .. } => arguments.is_empty(),
            CanonicalTypeKind::Primitive(_) => true,
            _ => false,
        };
        registered
            && declaration_identity
            && linked.type_surface_spellings.get(id).is_some_and(|registered_spelling| {
                let registered_spelling = registered_spelling.as_str();
                registered_spelling == spelling || registered_spelling.rsplit("::").next() == Some(short)
            })
    })
}

/// 验证整个类型闭包，不限于当前函数引用到的行。
pub(crate) fn validate_linked_types(linked: &LinkedSemanticProgram) -> Result<(), ValkyrieMirError> {
    for (owner, record) in &linked.types {
        let references = match &record.kind {
            CanonicalTypeKind::Primitive(primitive) => {
                let valid = match primitive {
                    CanonicalPrimitiveType::Integer { bits, .. } => matches!(bits, 8 | 16 | 32 | 64 | 128),
                    CanonicalPrimitiveType::Float { bits } => matches!(bits, 32 | 64),
                    _ => true,
                };
                if !valid {
                    return Err(ValkyrieMirError::InvalidPrimitiveType { ty: *owner });
                }
                Vec::new()
            }
            CanonicalTypeKind::Nominal { declaration, arguments } => {
                std::iter::once(*declaration).chain(arguments.iter().copied()).collect()
            }
            CanonicalTypeKind::Tuple(members) | CanonicalTypeKind::Union(members) | CanonicalTypeKind::Intersection(members) => members.clone(),
            CanonicalTypeKind::LabeledTuple { fields } => fields.iter().map(|(_, ty)| *ty).collect(),
            CanonicalTypeKind::Array { element, .. } | CanonicalTypeKind::Nullable(element) => {
                vec![*element]
            }
            CanonicalTypeKind::Function { parameters, return_type } => {
                parameters.iter().copied().chain(std::iter::once(*return_type)).collect()
            }
            // 形参不引用其他类型身份：binder(owner, ordinal) 本身就是稳定身份。
            CanonicalTypeKind::Generic { .. } => Vec::new(),
        };
        for referenced in references {
            if !linked.types.contains_key(&referenced) {
                return Err(ValkyrieMirError::UnknownTypeReference { owner: *owner, referenced });
            }
        }
    }
    let mut nominal_semantics = BTreeMap::new();
    let mut nominal_types = std::collections::BTreeSet::new();
    let mut declared_fields = std::collections::BTreeSet::new();
    for (nominal, record) in &linked.nominal_instances {
        if !matches!(linked.types.get(&record.declaration).map(|ty| &ty.kind), Some(CanonicalTypeKind::Nominal { declaration, .. }) if *declaration == record.declaration)
        {
            return Err(ValkyrieMirError::InvalidNominalDeclaration { nominal: *nominal, declaration: record.declaration });
        }
        if !matches!(linked.types.get(&record.ty).map(|ty| &ty.kind), Some(CanonicalTypeKind::Nominal { declaration, .. }) if *declaration == record.declaration)
        {
            return Err(ValkyrieMirError::InvalidNominalInstanceType { nominal: *nominal, ty: record.ty });
        }
        if !nominal_types.insert(record.ty) {
            return Err(ValkyrieMirError::DuplicateNominalInstanceType { ty: record.ty });
        }
        if let Some(previous) = nominal_semantics.insert(record.declaration, record.semantics) {
            if previous != record.semantics {
                return Err(ValkyrieMirError::ConflictingNominalSemantics { declaration: record.declaration });
            }
        }
        for field in &record.fields {
            if !declared_fields.insert(*field)
                || !linked.fields.get(field).is_some_and(|row| row.owner == *nominal && linked.types.contains_key(&row.ty))
            {
                return Err(ValkyrieMirError::InvalidNominalField { nominal: *nominal, field: *field });
            }
        }
    }
    for (field, record) in &linked.fields {
        if !declared_fields.contains(field) {
            return Err(ValkyrieMirError::InvalidNominalField { nominal: record.owner, field: *field });
        }
    }
    for record in linked.types.values() {
        if let CanonicalTypeKind::Nominal { declaration, .. } = &record.kind {
            if !nominal_semantics.contains_key(declaration) {
                return Err(ValkyrieMirError::MissingNominalSemantics { declaration: *declaration });
            }
        }
    }
    for (ty, record) in &linked.types {
        let references = match &record.kind {
            CanonicalTypeKind::Nominal { declaration, arguments } => {
                if ty != declaration || !arguments.is_empty() {
                    validate_nominal_value_type(linked, *ty, &nominal_types)?;
                }
                arguments.clone()
            }
            CanonicalTypeKind::Tuple(members) | CanonicalTypeKind::Union(members) | CanonicalTypeKind::Intersection(members) => members.clone(),
            CanonicalTypeKind::LabeledTuple { fields } => fields.iter().map(|(_, ty)| *ty).collect(),
            CanonicalTypeKind::Array { element, .. } | CanonicalTypeKind::Nullable(element) => {
                vec![*element]
            }
            CanonicalTypeKind::Function { parameters, return_type } => {
                parameters.iter().copied().chain(std::iter::once(*return_type)).collect()
            }
            CanonicalTypeKind::Primitive(_) => Vec::new(),
            CanonicalTypeKind::Generic { .. } => Vec::new(),
        };
        for referenced in references {
            validate_nominal_value_type(linked, referenced, &nominal_types)?;
        }
    }
    for record in linked.item_instances.values() {
        for ty in record.parameter_types.iter().chain(std::iter::once(&record.return_type)) {
            validate_nominal_value_type(linked, *ty, &nominal_types)?;
        }
    }
    for field in linked.fields.values() {
        validate_nominal_value_type(linked, field.ty, &nominal_types)?;
    }
    for ((nominal, variant_id), variant) in &linked.variants {
        if !linked.nominal_instances.contains_key(nominal) {
            return Err(ValkyrieMirError::InvalidVariantOwner { nominal: *nominal, variant: *variant_id });
        }
        if let Some(ty) = variant.payload_type {
            validate_nominal_value_type(linked, ty, &nominal_types)?;
        }
    }
    for import in linked.imports.values() {
        let Some(callee) = linked.item_instances.get(&import.callee)
        else {
            return Err(ValkyrieMirError::UnknownFunction { function: import.callee });
        };
        if callee.parameter_types != import.parameter_types || callee.return_type != import.return_type {
            return Err(ValkyrieMirError::ImportSignatureMismatch { function: import.callee });
        }
        for ty in import.parameter_types.iter().chain(std::iter::once(&import.return_type)) {
            if !linked.types.contains_key(ty) {
                return Err(ValkyrieMirError::UnknownType { function: import.callee, ty: *ty });
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_nominal_value_type(
    linked: &LinkedSemanticProgram,
    ty: TypeId,
    nominal_types: &std::collections::BTreeSet<TypeId>,
) -> Result<(), ValkyrieMirError> {
    if matches!(linked.types.get(&ty).map(|record| &record.kind), Some(CanonicalTypeKind::Nominal { .. })) && !nominal_types.contains(&ty) {
        return Err(ValkyrieMirError::MissingNominalInstance { ty });
    }
    Ok(())
}
