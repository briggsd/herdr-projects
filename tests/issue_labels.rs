//! Labels taken off the issue once the pull request merges (pr.clearIssueLabels): those the issue has are removed and
//! said in runner.log, through the board's own merge and a merge seen on GitHub; gh failing is said and holds nothing.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::actions::finish_line;
use sw::housekeeping::poll_prs;
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing";

/// A task at the gate with a pull request, implementing issue 12, with gh saying `gh`.
fn at_gate(gh: Value) -> (World, String) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never", "pr.clearIssueLabels": ["status:in-progress", "status:ready"] }), ..Default::default() });
    let id = to_approval(&w, "Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    w.gh_says(gh);
    (w, id)
}
fn log_of(id: &str) -> String { std::fs::read_to_string(sw::store::files::dir_of(id).join("runner.log")).unwrap_or_default() }
fn edits(w: &World) -> Vec<String> { gh_log(w).lines().filter(|l| l.starts_with("issue edit")).map(String::from).collect() }

#[test]
fn the_boards_merge_takes_the_status_labels_the_issue_has_off_it() {
    let (w, id) = at_gate(json!({ "labels": ["kind:bug", "status:in-progress", "p2"] }));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(edits(&w), vec!["issue edit 12 --remove-label status:in-progress"]);
    assert!(log_of(&id).contains("issue 12: removed status:in-progress"), "{}", log_of(&id));
}

#[test]
fn a_merge_seen_on_github_takes_them_off_too() {
    let (w, id) = at_gate(json!({ "labels": ["status:ready", "status:in-progress"], "view": { "state": "MERGED" } }));
    update(&id, |c| { c.put("stage", "merging"); c.put("autoMerge", true); c.put("autoMergeLocal", true); Ok(Change::Write) }).unwrap();
    poll_prs();
    assert_eq!(task(&id).s("stage"), "done");
    assert_eq!(edits(&w), vec!["issue edit 12 --remove-label status:in-progress", "issue edit 12 --remove-label status:ready"]);
    let log = log_of(&id);
    assert!(log.contains("issue 12: removed status:in-progress") && log.contains("issue 12: removed status:ready"), "{log}");
}

#[test]
fn gh_failing_is_said_and_the_task_is_done_all_the_same() {
    let (w, id) = at_gate(json!({ "labels": ["status:in-progress"], "issueEditFail": true }));
    finish_line::approve(&id, None, false).unwrap();
    let t = settle(&id);
    assert_eq!(t.s("stage"), "done");
    assert_eq!(edits(&w).len(), 1);
    assert!(log_of(&id).contains("issue 12: could not remove status:in-progress: HTTP 403"), "{}", log_of(&id));
}

#[test]
fn an_issue_without_them_is_left_alone() {
    let (w, id) = at_gate(json!({ "labels": ["kind:bug"] }));
    finish_line::approve(&id, None, false).unwrap();
    assert_eq!(settle(&id).s("stage"), "done");
    assert!(edits(&w).is_empty() && !log_of(&id).contains("issue 12"), "{}", log_of(&id));
}
