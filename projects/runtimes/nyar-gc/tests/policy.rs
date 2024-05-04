use nyar_gc::*;

#[test]
fn derived_budgets_tighten_with_pause_hint() {
    let tight = WorkloadHints { pause_budget_ms: Some(1), ..WorkloadHints::default() };
    assert_eq!(tight.derived_gray_budget(), Some(8));
    assert_eq!(tight.derived_trace_slices_per_poll(), Some(2));

    let explicit = WorkloadHints {
        concurrent_gray_budget: Some(3),
        concurrent_trace_slices_per_poll: Some(1),
        pause_budget_ms: Some(1),
        ..WorkloadHints::default()
    };
    assert_eq!(explicit.derived_gray_budget(), Some(3));
    assert_eq!(explicit.derived_trace_slices_per_poll(), Some(1));
}
