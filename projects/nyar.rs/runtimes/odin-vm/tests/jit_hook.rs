//! Consumer-side workload evidence tests: the JSON intent pipeline (`workload_json`) stays in
//! this crate while the engine substrate lives in Boost `interpreter-vm-ovm`.

use interpreter_gc::GcMode;
use odin_vm::{OdinVm, workload_json::parse_workload_intent_json};

#[test]
fn concurrent_interactive_fixture_wires_vm_evidence_accessors() {
    use odin_vm::workload_json::parse_workload_intent_json;

    let text = include_str!("../fixtures/workload/concurrent-interactive.json");
    let intent = parse_workload_intent_json(text).expect("parse");
    let mut vm = OdinVm::new();
    let decision = vm.apply_workload_intent(intent).expect("apply");
    assert_eq!(decision.mode, GcMode::ConcurrentMarkReserved);
    assert_eq!(decision.scenario_id.as_deref(), Some("concurrent-interactive"));
    assert!(vm.strategy_transition_history().len() >= 1);
    let _ = vm.last_trace_poll();
    let _ = vm.last_root_handshake();
    let _ = vm.last_relocate_map();
    assert!(vm.last_promotion_failure().is_none());
}

#[test]
fn gc_evidence_snapshot_includes_scenario_and_trace_fields() {
    use odin_vm::workload_json::{parse_workload_intent_json, snapshot_gc_evidence};

    let text = include_str!("../fixtures/workload/concurrent-interactive.json");
    let intent = parse_workload_intent_json(text).expect("parse");
    let mut vm = OdinVm::new();
    vm.apply_workload_intent(intent).expect("apply");
    let snap = snapshot_gc_evidence(&vm);
    assert_eq!(snap["decision"]["scenario_id"], "concurrent-interactive");
    assert_eq!(snap["decision"]["mode"], "concurrent_mark_reserved");
    assert!(snap["transitions"].as_array().expect("transitions").len() >= 1);
    assert!(snap["trace_poll"]["slices_run"].is_number());
    assert!(snap["root_handshake"]["stack_slots"].is_number());
    assert!(snap["max_trace_slices_per_poll"].is_number());
    assert_eq!(snap["relocate_has_moves"], false);
    assert!(snap["promotion_failure"].is_null());
}

#[test]
fn online_request_fixture_applies_generational_strategy_on_vm() {
    use odin_vm::workload_json::parse_workload_intent_json;

    let text = include_str!("../fixtures/workload/online-request.json");
    let intent = parse_workload_intent_json(text).expect("parse");
    let mut vm = OdinVm::new();
    let decision = vm.apply_workload_intent(intent).expect("apply");
    assert_eq!(decision.mode, GcMode::GenerationalLowLatency);
    assert_eq!(decision.scenario_id.as_deref(), Some("online-request"));
    assert_eq!(decision.hints.pause_budget_ms, Some(5));
    assert!(!decision.hints.allow_heavy_collection);
    assert!(vm.strategy_transition_history().len() >= 1);
}

#[test]
fn resident_service_fixture_applies_generational_strategy_on_vm() {
    use odin_vm::workload_json::parse_workload_intent_json;

    let text = include_str!("../fixtures/workload/resident-service.json");
    let intent = parse_workload_intent_json(text).expect("parse");
    let mut vm = OdinVm::new();
    let decision = vm.apply_workload_intent(intent).expect("apply");
    assert_eq!(decision.mode, GcMode::GenerationalLowLatency);
    assert_eq!(decision.scenario_id.as_deref(), Some("resident-service"));
    assert_eq!(decision.hints.pause_budget_ms, Some(10));
    assert_eq!(decision.hints.heap_soft_limit_bytes, Some(268435456));
    assert!(!decision.hints.allow_heavy_collection);
    assert!(vm.strategy_transition_history().len() >= 1);
}

#[test]
fn offline_batch_fixture_applies_throughput_strategy_on_vm() {
    use odin_vm::workload_json::parse_workload_intent_json;

    let text = include_str!("../fixtures/workload/offline-batch.json");
    let intent = parse_workload_intent_json(text).expect("parse");
    let mut vm = OdinVm::new();
    let decision = vm.apply_workload_intent(intent).expect("apply");
    assert_eq!(decision.mode, GcMode::ThroughputBatch);
    assert_eq!(decision.scenario_id.as_deref(), Some("offline-batch"));
    assert_eq!(decision.hints.pause_budget_ms, Some(200));
    assert_eq!(decision.hints.heap_soft_limit_bytes, Some(536870912));
    assert!(decision.hints.allow_heavy_collection);
    assert!(vm.strategy_transition_history().len() >= 1);
}

#[test]
fn nested_workload_phase_overrides_scenario_then_restores() {
    use interpreter_gc::IntentSource;
    use odin_vm::workload_json::parse_workload_intent_json;

    let batch = parse_workload_intent_json(include_str!("../fixtures/workload/offline-batch.json")).expect("batch");
    let mut request = parse_workload_intent_json(include_str!("../fixtures/workload/online-request.json")).expect("request");
    request.source = IntentSource::PhaseEvent;

    let mut vm = OdinVm::new();
    let outer = vm.apply_workload_intent(batch).expect("apply batch");
    assert_eq!(outer.mode, GcMode::ThroughputBatch);
    assert_eq!(outer.scenario_id.as_deref(), Some("offline-batch"));

    let nested = vm.begin_workload_phase(request).expect("begin request");
    assert_eq!(nested.mode, GcMode::GenerationalLowLatency);
    assert_eq!(nested.scenario_id.as_deref(), Some("online-request"));
    assert_eq!(nested.hints.pause_budget_ms, Some(5));
    assert!(vm.strategy_transition_history().len() >= 2);

    let restored = vm.end_workload_phase(Some("request")).expect("end request");
    assert_eq!(restored.mode, GcMode::ThroughputBatch);
    assert_eq!(restored.scenario_id.as_deref(), Some("offline-batch"));
    assert_eq!(vm.last_strategy_decision().map(|d| d.scenario_id.as_deref()), Some(Some("offline-batch")));
}
