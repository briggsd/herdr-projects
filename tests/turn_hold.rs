//! A turn that ends with the agent's own background job still running is held: nothing committed, merged or moved on,
//! runner.log says so, and the agent is told once to wait for it. Its next turn's end, with nothing left, goes on.
//! Its program's helpers do not count; after three holds in a row, or with a job over an hour, the turn goes on.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

fn running() -> Value { json!({ "tool": true }) }
fn none() -> Value { json!({ "tool": false }) }
/// The task once it settles, its runner.log, and what its agent was told.
fn ran(steps: Value, age: Option<i64>) -> (sw::store::tasks::Task, String, Vec<String>) { let (t, log, told, _) = ran_commits(steps, age); (t, log, told) }
/// The same, with the messages of the commits on its branch.
fn ran_commits(steps: Value, age: Option<i64>) -> (sw::store::tasks::Task, String, Vec<String>, Vec<String>) {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    if let Some(a) = age { w.patch(move |s| s["toolAge"] = json!(a)); }
    w.script(json!({ "worker": steps }));
    let id = w.add("Fix the thing");
    delegate(&id);
    let t = settle(&id);
    let log = std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default();
    let told = prompts(&w, t.at("link").s("pane"));
    let out = git(&w.root, &["log", "--format=%B%x00", &format!("{}..{}", t.s("baseSha"), t.s("branch"))]);
    let commits = out.split('\0').map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    (t, log, told, commits)
}

#[test]
fn a_turn_ended_with_its_background_job_running_is_held_and_the_agent_told_to_wait() {
    let (t, log, told, commits) = ran_commits(json!([
        { "files": { "f.txt": "v1\n", "tools/wait_for.sh": "#!/bin/sh\n" }, "note": "Did v1.", "pane": running() },
        { "note": "Waited: the check passed.", "pane": none() },
    ]), None);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert!(log.contains("turn end held: 2 process 2"), "{log}");
    assert_eq!(told.len(), 2, "told once: {told:?}");
    assert!(told[1].starts_with("A background job you started is still running: process 2 (pid 2). Wait for it with bash tools/wait_for.sh 2 <seconds>, then end your turn."), "{}", told[1]);
    // Nothing was committed at the held turn's end: the board's one commit is made after the wait.
    assert_eq!(commits.len(), 1, "{commits:?}");
    assert_eq!(t.s("summary"), "Did v1.\n\nWaited: the check passed.", "the summary keeps the held turn's words");
    assert!(!t.b("turnHolds") && !t.b("heldSaid"), "a turn that goes on clears the holds");
}

#[test]
fn without_the_repo_s_waiter_the_agent_is_told_to_wait_on_the_pid() {
    let (t, _, told) = ran(json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "pane": running() }, { "note": "Waited.", "pane": none() }]), None);
    assert_eq!(t.s("stage"), "approval");
    assert!(told[1].contains("Wait for it to end (while kill -0 2 2>/dev/null; do sleep 5; done), then end your turn."), "{}", told[1]);
}

#[test]
fn a_turn_with_nothing_left_running_or_only_its_helpers_goes_on_at_once() {
    for pane in [none(), json!({ "helper": true })] {
        let (t, log, told) = ran(json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "pane": pane }]), None);
        assert_eq!(t.s("stage"), "approval", "{pane}");
        assert!(!log.contains("turn end held"), "{log}");
        assert_eq!(told.len(), 1, "{told:?}");
    }
}

#[test]
fn after_three_holds_or_with_a_job_over_an_hour_the_turn_goes_on_and_says_so() {
    let held = |n: usize| (0..n).map(|i| json!({ "files": { "f.txt": format!("v{i}\n") }, "commit": format!("v{i}"), "note": format!("Turn {i}."), "pane": running() })).collect::<Vec<_>>();
    let (t, log, told) = ran(json!(held(4)), None);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(log.matches("turn end held: 2 process 2").count(), 3, "{log}");
    assert!(log.contains("turn end not held: held 3 times already; going on with 2 process 2 still running"), "{log}");
    assert_eq!(told.len(), 4, "{told:?}");
    let (t, log, told) = ran(json!(held(1)), Some(3700));
    assert_eq!(t.s("stage"), "approval");
    assert!(log.contains("turn end not held: 2 has run 61 min; going on with 2 process 2 still running") && !log.contains("turn end held"), "{log}");
    assert_eq!(told.len(), 1);
}

#[test]
fn a_process_gone_by_the_time_it_is_read_or_a_second_later_holds_nothing() {
    for pane in [json!({ "tool": true, "toolGone": true }), json!({ "tool": true, "toolBrief": true })] {
        let (t, log, told) = ran(json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Did v1.", "pane": pane }]), None);
        assert_eq!(t.s("stage"), "approval", "{pane}: {} {}", t.s("msg"), t.at("failure"));
        assert!(!log.contains("turn end held"), "{pane}: {log}");
        assert_eq!(told.len(), 1, "{pane}: {told:?}");
    }
}
