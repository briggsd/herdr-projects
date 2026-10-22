//! An approval is for the commits you saw: a fix the agent commits after it (the check failed on the branch brought
//! up to date at the merge) drops the approval, and the task asks again; a merge of main alone keeps it.
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;

fn main_moves(w: &World, file: &str, text: &str) { std::fs::write(w.root.join(file), text).unwrap(); git(&w.root, &["add", "-A"]); git(&w.root, &["commit", "-qm", "main moves"]); }
/// A task that passed its review and waits for approval; the check fails while other.txt says BROKEN.
fn waiting() -> (World, String) {
    let w = world_with(Opts { repo: true, repo_settings: Some(json!({ "check.command": "! grep -qs BROKEN other.txt" })), ..Default::default() });
    w.script(json!({ "worker": [{ "files": { "task.txt": "TASK\n" }, "commit": "task", "note": "Added task.txt." }] }));
    let id = w.add("Add task.txt");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure"));
    (w, id)
}
fn log_of(id: &str) -> String { std::fs::read_to_string(sw::store::files::dir_of(id).join("runner.log")).unwrap_or_default() }

#[test]
fn a_fix_committed_after_the_approval_drops_it_and_the_task_asks_again() {
    let (w, id) = waiting();
    main_moves(&w, "other.txt", "BROKEN\n");
    w.script(json!({ "worker": [{ "files": { "other.txt": "fixed\n" }, "commit": "fix the check", "note": "Fixed the check." }] }));
    sw::actions::finish_line::approve(&id, None, false).unwrap();
    let mut t = settle(&id); for _ in 0..3 { t = settle(&id); }
    assert_eq!(t.s("stage"), "approval", "not merged on the old approval: {} {}", t.s("msg"), t.at("failure"));
    assert!(!t.b("mergeIntent") && t.b("requireApproval"), "{t:?}");
    assert!(t.s("msg").starts_with("Approve to merge?") && t.s("msg").contains("Changed after your approval: ") && t.s("msg").contains("fix the check"), "{}", t.s("msg"));
    assert!(log_of(&id).contains("approval dropped: 1 commit after it: ") && log_of(&id).contains(" fix the check"), "{}", log_of(&id));
    assert_eq!(read(&w.root.join("other.txt")), "BROKEN\n", "main is as it was");
    sw::actions::finish_line::approve(&id, None, false).unwrap();
    assert_eq!(settle(&id).s("stage"), "done");
    assert_eq!(read(&w.root.join("other.txt")), "fixed\n");
}

#[test]
fn a_merge_of_main_alone_keeps_the_approval() {
    let (w, id) = waiting();
    main_moves(&w, "other.txt", "MAIN\n");
    sw::actions::finish_line::approve(&id, None, false).unwrap();
    let mut t = settle(&id); for _ in 0..2 { t = settle(&id); }
    assert_eq!(t.s("stage"), "done", "{} {}", t.s("msg"), t.at("failure"));
    assert!(!log_of(&id).contains("approval dropped"), "{}", log_of(&id));
    assert_eq!(read(&w.root.join("task.txt")), "TASK\n");
}
