use nyar_gc::*;

#[test]
fn disabled_controller_rejects_begin() {
    let mut ctrl = ConcurrentMarkController::new();
    assert!(!ctrl.enabled());
    assert!(ctrl.transition(ConcurrentMarkEvent::BeginCycle).is_err());
}

#[test]
fn happy_path_reaches_idle() {
    let mut ctrl = ConcurrentMarkController::new();
    ctrl.set_enabled(true);
    ctrl.transition(ConcurrentMarkEvent::BeginCycle).unwrap();
    ctrl.transition(ConcurrentMarkEvent::RootsReady).unwrap();
    ctrl.transition(ConcurrentMarkEvent::TraceSliceDone).unwrap();
    ctrl.transition(ConcurrentMarkEvent::TraceSliceDone).unwrap();
    ctrl.transition(ConcurrentMarkEvent::TerminationOk).unwrap();
    ctrl.transition(ConcurrentMarkEvent::TerminationOk).unwrap();
    ctrl.transition(ConcurrentMarkEvent::RemarkDone).unwrap();
    ctrl.transition(ConcurrentMarkEvent::SweepDone).unwrap();
    assert_eq!(ctrl.state(), ConcurrentMarkState::Idle);
    assert_eq!(ctrl.cycles_started(), 1);
}

#[test]
fn abort_returns_to_idle() {
    let mut ctrl = ConcurrentMarkController::new();
    ctrl.set_enabled(true);
    ctrl.transition(ConcurrentMarkEvent::BeginCycle).unwrap();
    ctrl.transition(ConcurrentMarkEvent::Abort).unwrap();
    assert_eq!(ctrl.state(), ConcurrentMarkState::Idle);
}

#[test]
fn satb_required_only_during_mutator_visible_mark() {
    let mut ctrl = ConcurrentMarkController::new();
    assert!(!ctrl.requires_satb());
    ctrl.set_enabled(true);
    ctrl.transition(ConcurrentMarkEvent::BeginCycle).unwrap();
    assert!(!ctrl.requires_satb());
    ctrl.transition(ConcurrentMarkEvent::RootsReady).unwrap();
    ctrl.transition(ConcurrentMarkEvent::TraceSliceDone).unwrap();
    assert!(ctrl.requires_satb());
    assert_eq!(ctrl.state(), ConcurrentMarkState::ConcurrentTrace);
}
