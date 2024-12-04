use nyar_gc::*;

#[test]
fn empty_controller_uses_conservative_baseline() {
    let mut ctrl = StrategyController::new();
    let decision = ctrl.decide();
    assert_eq!(decision.mode, GcMode::MarkSweep);
    assert!(decision.unknown_dimensions.contains(&"pause_budget_ms"));
}

#[test]
fn online_sample_selects_generational() {
    let mut ctrl = StrategyController::new();
    ctrl.set_process_intent(WorkloadIntent::sample_online_request()).unwrap();
    let decision = ctrl.decide();
    assert_eq!(decision.mode, GcMode::GenerationalLowLatency);
    assert_eq!(decision.hints.pause_budget_ms, Some(5));
    assert!(!decision.hints.allow_heavy_collection);
    assert_eq!(decision.scenario_id.as_deref(), Some("online-request"));
}

#[test]
fn nested_phase_takes_stricter_pause() {
    let mut ctrl = StrategyController::new();
    ctrl.set_process_intent(WorkloadIntent::sample_offline_batch()).unwrap();
    let mut request = WorkloadIntent::sample_online_request();
    request.source = IntentSource::PhaseEvent;
    ctrl.begin_phase(request).unwrap();
    let decision = ctrl.decide();
    assert_eq!(decision.hints.pause_budget_ms, Some(5));
    assert_eq!(decision.mode, GcMode::GenerationalLowLatency);
    assert!(!decision.hints.allow_heavy_collection);
}

#[test]
fn hard_conflict_rejects_throughput_with_tight_pause() {
    let err = StrategyController::new().set_process_intent(WorkloadIntent::sample_hard_conflict_throughput_tight_pause()).unwrap_err();
    assert!(matches!(err, IntentError::HardConflict { .. }));
}

#[test]
fn hard_conflict_rejects_concurrent_with_heavy() {
    let err = StrategyController::new().set_process_intent(WorkloadIntent::sample_hard_conflict_concurrent_heavy()).unwrap_err();
    assert!(matches!(err, IntentError::HardConflict { .. }));
}

#[test]
fn concurrent_interactive_sample_applies() {
    let mut ctrl = StrategyController::new();
    ctrl.set_process_intent(WorkloadIntent::sample_concurrent_interactive()).unwrap();
    let decision = ctrl.decide();
    assert_eq!(decision.mode, GcMode::ConcurrentMarkReserved);
    assert_eq!(decision.hints.pause_budget_ms, Some(5));
    assert!(!decision.hints.allow_heavy_collection);
    assert_eq!(decision.scenario_id.as_deref(), Some("concurrent-interactive"));
    assert!(!decision.unknown_dimensions.contains(&"scenario"));
}

#[test]
fn nested_phase_scenario_overrides_process() {
    let mut ctrl = StrategyController::new();
    ctrl.set_process_intent(WorkloadIntent::sample_offline_batch()).unwrap();
    let mut request = WorkloadIntent::sample_online_request();
    request.source = IntentSource::PhaseEvent;
    ctrl.begin_phase(request).unwrap();
    let decision = ctrl.decide();
    assert_eq!(decision.scenario_id.as_deref(), Some("online-request"));
}

#[test]
fn transition_history_records_mode_changes_only() {
    let mut ctrl = StrategyController::new();
    let first = ctrl.decide();
    assert_eq!(first.mode, GcMode::MarkSweep);
    assert_eq!(ctrl.transition_history().len(), 1);
    assert_eq!(ctrl.transition_history()[0].from_mode, None);

    ctrl.decide();
    assert_eq!(ctrl.transition_history().len(), 1, "same mode must not append");

    ctrl.set_process_intent(WorkloadIntent::sample_online_request()).unwrap();
    let second = ctrl.decide();
    assert_eq!(second.mode, GcMode::GenerationalLowLatency);
    assert_eq!(ctrl.transition_history().len(), 2);
    assert_eq!(ctrl.transition_history()[1].from_mode, Some(GcMode::MarkSweep));
    assert_eq!(ctrl.transition_history()[1].decision.mode, GcMode::GenerationalLowLatency);
    assert!(ctrl.transition_history()[1].decision.reason.contains("merged"));
}

#[test]
fn end_phase_checks_name() {
    let mut ctrl = StrategyController::new();
    ctrl.begin_phase(WorkloadIntent::sample_online_request()).unwrap();
    let err = ctrl.end_phase(Some("batch")).unwrap_err();
    assert!(matches!(err, IntentError::PhaseMismatch { .. }));
    ctrl.end_phase(Some("request")).unwrap();
    assert_eq!(ctrl.phase_depth(), 0);
}

#[test]
fn apply_to_policy_updates_mode_and_hints() {
    let mut ctrl = StrategyController::new();
    ctrl.set_process_intent(WorkloadIntent::sample_resident_service()).unwrap();
    let mut policy = GcPolicy::mark_sweep_baseline();
    let decision = ctrl.apply_to_policy(&mut policy);
    assert_eq!(policy.mode, GcMode::GenerationalLowLatency);
    assert_eq!(policy.hints.heap_soft_limit_bytes, Some(256 * 1024 * 1024));
    assert_eq!(decision.mode, policy.mode);
}
