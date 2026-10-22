//! Making a task's worktree while another git command holds .git/config.lock (FL-331): the empty branch it left is
//! deleted and the create is tried again, said in runner.log; a lock that stays fails the start after three tries.
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;

fn log_of(id: &str) -> String { std::fs::read_to_string(sw::store::files::dir_of(id).join("runner.log")).unwrap_or_default() }
fn locked(times: i64) -> (World, String) {
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.patch(|s| s["worktreeLocked"] = json!(times));
    let id = w.add("Fix the thing");
    delegate(&id);
    (w, id)
}

#[test]
fn a_lock_in_the_way_once_is_tried_again_and_the_task_starts_in_its_worktree() {
    let (_w, id) = locked(1);
    let t = settle(&id);
    assert!(t.at("failure").is_null() && t.b("branch") && t.b("baseSha"), "{} {}", t.s("stage"), t.at("failure"));
    let line = "worktree: a git lock was in the way (could not lock config file .git/config: File exists), trying again; its empty branch is deleted";
    assert_eq!(log_of(&id).matches(line).count(), 1, "{}", log_of(&id));
}

#[test]
fn a_lock_that_stays_fails_the_start_after_three_tries() {
    let (_w, id) = locked(5);
    let t = settle(&id);
    assert!(!t.b("worktree"), "{}", t.s("worktree"));
    assert_eq!(log_of(&id).matches("worktree: a git lock was in the way").count(), 2, "{}", log_of(&id));
    assert!(log_of(&id).contains("could not lock config file"), "{}", log_of(&id));
}
