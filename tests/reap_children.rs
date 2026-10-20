//! Local patch: a detached child that ends is reaped, not left a zombie.
use std::process::Command;

#[test]
fn ended_detached_children_are_reaped() {
    let before = sw_flow::runner::kept();
    let mut c = Command::new("true");
    sw_flow::runner::detach(&mut c);
    let pid = sw_flow::runner::keep(c.spawn());
    assert!(pid > 0);
    assert_eq!(sw_flow::runner::kept(), before + 1);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while sw_flow::runner::kept() > before && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
        sw_flow::runner::reap();
    }
    assert_eq!(sw_flow::runner::kept(), before, "the ended child is still held");
    // Waited for, its pid is gone: no zombie is left (kill 0 fails with ESRCH).
    assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
}

#[test]
fn a_child_that_did_not_start_is_not_held() {
    let before = sw_flow::runner::kept();
    assert_eq!(sw_flow::runner::keep(Command::new("/nonexistent/program").spawn()), 0);
    assert_eq!(sw_flow::runner::kept(), before);
}
