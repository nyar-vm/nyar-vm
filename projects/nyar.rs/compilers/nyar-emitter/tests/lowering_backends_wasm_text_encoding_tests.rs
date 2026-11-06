use std::collections::BTreeMap;

use nyar::{HostProjectionBoundary, NyarType, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, ExecutableFunction, Terminator},
    lowering::backends::wasm::validate_text_encoding_projection,
};

fn utf16_submission() -> FragmentSubmission {
    text_abi_submission(NyarType::Utf16)
}

fn text_abi_submission(param_type: NyarType) -> FragmentSubmission {
    let operation = QualifiedName::new(vec![nyar::Identifier::new("entry")]);
    let function = ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: "entry".to_string(),
        return_type: NyarType::Unit,
        param_types: vec![param_type],
        value_types: BTreeMap::new(),
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
        diagnostics: Vec::new(),
    };
    FragmentSubmission::from_backend_plan(TargetExecutionPlan::from_functions(BTreeMap::from([(operation, function)])))
}

#[test]
fn rejects_utf16_without_a_declared_wasm_carrier_on_wasi() {
    let error = validate_text_encoding_projection(&utf16_submission(), HostProjectionBoundary::WasiComponent)
        .expect_err("canonical ABI 没有 `utf16` 表示");
    let message = error.to_string();
    assert!(message.contains("WASM pre-emission verifier"), "{message}");
    assert!(message.contains("explicit UTF-16 ABI contract"), "{message}");
}

#[test]
fn admits_declared_utf16_abi_sites_on_wasm_js_glue() {
    validate_text_encoding_projection(&utf16_submission(), HostProjectionBoundary::WasmJsGlue)
        .expect("`WasmJsGlue` 已冻结 `utf16` 编码身份，verifier 不得再按类型拒绝");
}

#[test]
fn keeps_nested_utf16_denied_without_a_declared_abi_position() {
    let submission = text_abi_submission(NyarType::Tuple(vec![NyarType::Utf16]));
    let error = validate_text_encoding_projection(&submission, HostProjectionBoundary::WasmJsGlue)
        .expect_err("聚合内的 `utf16` 没有独立的 ABI 位置");
    let message = error.to_string();
    assert!(message.contains("requires an explicit UTF-16 ABI contract"), "{message}");
    assert!(message.contains("must not use the UTF-8 carrier"), "{message}");
}
