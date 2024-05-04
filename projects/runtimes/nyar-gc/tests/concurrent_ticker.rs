use nyar_gc::*;

#[test]
fn ticker_increments_while_running() {
    let mut ticker = ConcurrentMarkTicker::new();
    assert!(!ticker.running());
    ticker.start(Duration::from_millis(5));
    assert!(ticker.running());
    thread::sleep(Duration::from_millis(40));
    let n = ticker.ticks();
    assert!(n >= 1, "expected at least one tick, got {n}");
    ticker.stop();
    assert!(!ticker.running());
    let after = ticker.ticks();
    thread::sleep(Duration::from_millis(20));
    assert_eq!(ticker.ticks(), after, "ticks must freeze after stop");
}
