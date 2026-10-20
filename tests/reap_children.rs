//! Local patch: a detached child that ends is reaped, not left a zombie (runner::keep, runner::reap).
use std::process::Command;

#[test]
fn an_ended_detached_child_is_reaped() {
    let mut c = Command::new("true");
    sw::runner::detach(&mut c);
    let pid = sw::runner::keep(c.spawn()) as i32;
    assert!(pid > 0);
    // A zombie still answers kill 0; once it is waited for, its pid is gone (ESRCH).
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while unsafe { libc::kill(pid, 0) } == 0 && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(20));
        sw::runner::reap();
    }
    assert_eq!(unsafe { libc::kill(pid, 0) }, -1, "the ended child was left a zombie");
}

#[test]
fn a_child_that_did_not_start_gives_no_pid() {
    assert_eq!(sw::runner::keep(Command::new("/nonexistent/program").spawn()), 0);
}
