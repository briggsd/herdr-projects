//! A task that waits (approval) is not re-armed by its agent working on its own: Claude Code compacting its
//! conversation while idle shows as `working` with no prompt of yours in it. Only a prompt since herdr last saw the
//! agent work makes a turn yours. And a chat with reviewed work that ends without a summary and changed no code puts
//! the task back where it was, its passed review kept, instead of stopping it without a summary.
use crate::common;
use common::*;
use serde_json::{json, Value};
use std::path::Path;
use sw::herdr::Herdr;
use sw::json::*;
use sw::store::tasks::{update, Change};

fn herdr_says(w: &World, pane: &str, status: &str, complete: bool) {
    w.patch(|s| { let seq = s.i("seq") + 1; s["seq"] = json!(seq); for p in s["panes"].as_array_mut().unwrap() { if p.s("pane_id") == pane { p["agent"]["status"] = json!(status); p["agent"]["seq"] = json!(seq); if complete { p["agent"]["completion"] = json!(seq); } } } });
}
fn line(file: &Path, v: Value) { use std::io::Write; std::fs::OpenOptions::new().create(true).append(true).open(file).unwrap().write_all(format!("{v}\n").as_bytes()).unwrap(); }
fn iso(ms: i64) -> String { sw::clock::iso(ms) }

#[test]
fn an_agent_compacting_while_idle_at_approval_leaves_the_task_at_approval_and_a_prompt_of_yours_still_counts() {
    let w = world();
    unsafe { std::env::set_var("HERDR_PROJECTS_PROMPT_WAIT_MS", "1") };
    let id = to_approval(&w, "Work");
    let before = task(&id);
    let pane = before.at("link").t("pane");
    assert_eq!(before.s("stage"), "approval");
    let worked = sw::clock::now() - 60_000;
    // its conversation: the last prompt and its finished turn, before herdr last saw it work; then a compaction
    let said = w.base.join("said.jsonl");
    line(&said, json!({ "type": "user", "uuid": "u1", "timestamp": iso(worked - 5_000), "message": { "content": "Fix the review's findings." } }));
    line(&said, json!({ "type": "assistant", "uuid": "a1", "timestamp": iso(worked - 1_000), "message": { "id": "m1", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Done." }] } }));
    line(&said, json!({ "type": "system", "subtype": "compact_boundary", "uuid": "c1", "timestamp": iso(sw::clock::now()), "compactMetadata": { "trigger": "auto" } }));
    line(&said, json!({ "type": "user", "uuid": "u2", "isCompactSummary": true, "timestamp": iso(sw::clock::now()), "message": { "content": "This session is being continued from a previous conversation..." } }));
    let sp = said.display().to_string();
    update(&id, |t| { t.put("transcript", sp); t.put("workedAt", worked); Ok(Change::Write) }).unwrap();
    // herdr sees it at work, then at rest with one more finished turn: nothing changes
    herdr_says(&w, &pane, "working", false);
    on_pane(&w, &pane);
    herdr_says(&w, &pane, "done", true);
    on_pane(&w, &pane);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert!(!t.b("chat") && !t.at("review").b("stale") && !t.b("turnOpen") && !t.b("unprompted"), "{} {}", t.at("chat"), t.at("review"));
    assert_eq!((t.at("reviewedHead"), t.at("requireApproval")), (before.at("reviewedHead"), before.at("requireApproval")));
    // a prompt you typed after that: your turn, as always
    line(&said, json!({ "type": "user", "uuid": "u3", "timestamp": iso(sw::clock::now() + 1_000), "message": { "content": "And the tests?" } }));
    herdr_says(&w, &pane, "working", false);
    on_pane(&w, &pane);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.b("turnOpen"), t.at("chat").s("via")), ("working", true, "terminal"));
    unsafe { std::env::remove_var("HERDR_PROJECTS_PROMPT_WAIT_MS") };
}

#[test]
fn a_chat_with_a_passed_task_that_ends_without_a_summary_and_changed_nothing_goes_back_to_approval() {
    let w = world();
    let id = to_approval(&w, "Work");
    let before = task(&id);
    let pane = before.at("link").t("pane");
    // typed in its terminal (nothing tells it was not you), and the turn ends with no summary and no closing note
    w.script(json!({ "worker": [{ "note": null, "status": "working" }] }));
    Herdr::new(Some(w.socket.clone())).agent_prompt(&pane, "what does it do?").unwrap();
    on_pane(&w, &pane);
    let t = task(&id);
    assert_eq!((t.s("stage"), t.at("chat").s("via")), ("working", "terminal"));
    herdr_says(&w, &pane, "done", true);
    on_pane(&w, &pane);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert!(!t.b("chat") && !t.at("review").b("stale") && !t.b("turnOpen"), "{} {}", t.at("chat"), t.at("review"));
    assert_eq!((t.at("reviewedHead"), t.at("review").at("text")), (before.at("reviewedHead"), before.at("review").at("text")));
    assert!(t.arr("reviews").last().unwrap().arr("kept").iter().any(|k| k.b("chat")), "the review still stands");
}
