//! The pull request opens from the agent's latest closing note, once it is a body (pr.bodyNeeds, pr.bodyRefuses): a
//! note without Checked, or one that says "no PR yet", is refused once and the next note opens it; one with Checked
//! opens it as before.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

/// The body the board opened the pull request with, the task's runner.log, and what the worker was told.
fn opened(steps: Value) -> (String, String, Vec<String>) { opened_by("pi", steps) }
fn opened_by(agent: &str, steps: Value) -> (String, String, Vec<String>) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "worker.agent": agent, "review.when": "never", "pr.bodyNeeds": ["Checked:"], "pr.bodyRefuses": ["no PR yet"] }), ..Default::default() });
    w.script(json!({ "worker": steps }));
    let id = w.add("Fix the thing");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let log = gh_log(&w);
    assert_eq!(log.matches("pr create ").count(), 1, "{log}");
    let body = log.split("pr create ").nth(1).unwrap().split_once(" --body ").unwrap().1.split("\nTask: ").next().unwrap().trim_end().to_string();
    (body, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default(), prompts(&w, t.at("link").s("pane")))
}

#[test]
fn a_note_with_checked_opens_the_pull_request_as_before() {
    let (body, log, told) = opened(json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: tools/check.sh passes." }]));
    assert_eq!(body, "Fixed it.\n\nChecked: tools/check.sh passes.");
    assert!(!log.contains("body refused") && told.len() == 1, "{log}");
}

#[test]
fn a_note_without_checked_is_refused_once_and_the_next_one_opens_it() {
    let (body, log, told) = opened(json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it; no PR yet." },
        { "note": "Fixed it.\n\nChecked: tools/check.sh passes.\n\nShots: none." },
    ]));
    assert!(log.contains("pr open: body refused (no Checked, says \"no PR yet\")"), "{log}");
    assert_eq!(told.len(), 2, "told once: {told:?}");
    assert!(told[1].starts_with("The board opens the pull request from your closing note, and that note is not a pull request body yet"), "{}", told[1]);
    assert_eq!(body, "Fixed it.\n\nChecked: tools/check.sh passes.\n\nShots: none.", "the latest note, not the refused one");
}

#[test]
fn refused_once_only_the_next_note_opens_it_as_it_is() {
    let (body, log, told) = opened(json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it." },
        { "note": "Still no checked line." },
    ]));
    assert_eq!(log.matches("body refused").count(), 1, "{log}");
    assert_eq!(told.len(), 2, "{told:?}");
    assert_eq!(body, "Still no checked line.");
}

#[test]
fn a_last_message_that_is_not_a_body_gives_way_to_the_turns_closing_note() {
    // FL-315: the closing note is in closing-note.md, and the last message only says so
    let (body, log, told) = opened_by("claude", json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: tools/check.sh passes.", "ends": "claude-finished-made", "hook": true }]));
    assert_eq!(body, "Fixed it.\n\nChecked: tools/check.sh passes.");
    assert!(log.contains("pr body: from closing-note.md") && !log.contains("body refused"), "{log}");
    assert_eq!(told.len(), 1, "{told:?}");
}

#[test]
fn neither_a_body_the_message_is_refused_as_before() {
    let (body, log, told) = opened_by("claude", json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.", "ends": "claude-finished-made", "hook": true },
        { "note": "Fixed it.\n\nChecked: tools/check.sh passes.", "ends": "claude-finished-made", "hook": true },
    ]));
    assert!(log.contains("pr open: body refused (no Checked)"), "{log}");
    assert_eq!(body, "Fixed it.\n\nChecked: tools/check.sh passes.");
    assert_eq!(told.len(), 2, "{told:?}");
}
