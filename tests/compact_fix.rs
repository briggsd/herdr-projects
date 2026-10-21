//! A fix round starts on a compacted conversation (worker.compactBeforeFix): the worker is told /compact, then the
//! findings, in that order; with the setting off nothing changes; a compact that fails still sends the findings.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::tasks::{update, Change};

fn failing() -> Value { json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No test.", "findings": [{ "id": "F1", "severity": "major", "subject": "No test", "where": "f.txt", "proof": "read it" }] } }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
/// A task sent back once and passed after its fix: the worker's prompts in order, and runner.log.
fn run(on: bool, compact_fails: bool) -> (Vec<String>, String) {
    let w = world_with(Opts { repo: true, settings: json!({ "worker.agent": "claude", "reviewer.agent": "claude", "worker.compactBeforeFix": on, "review.when": "always" }), ..Default::default() });
    if compact_fails { w.patch(|s| s["compactFails"] = json!(true)); }
    w.script(json!({ "worker": [step(1), step(2)], "reviewer": [failing(), passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", "Fix the thing"); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let worker_pane = t.at("link").t("pane");
    let prompts: Vec<String> = calls(&w).into_iter().filter(|c| c.starts_with(&format!("agent prompt {worker_pane} "))).collect();
    (prompts, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}
fn at(prompts: &[String], what: &str) -> Option<usize> { prompts.iter().position(|p| p.contains(what)) }

#[test]
fn a_fix_round_sends_compact_then_the_findings() {
    let (prompts, log) = run(true, false);
    let (c, f) = (at(&prompts, "/compact Keep: the issue number and acceptance list"), at(&prompts, "No test"));
    assert!(c.is_some() && f.is_some() && c < f, "{prompts:#?}");
    assert_eq!(prompts.iter().filter(|p| p.contains("/compact")).count(), 1, "{prompts:#?}");
    assert!(log.contains("fix round 1: compacted"), "{log}");
}

#[test]
fn with_the_setting_off_nothing_is_compacted() {
    let (prompts, log) = run(false, false);
    assert!(at(&prompts, "/compact").is_none() && at(&prompts, "No test").is_some(), "{prompts:#?}");
    assert!(!log.contains("compact"), "{log}");
}

#[test]
fn a_compact_that_fails_still_sends_the_findings_and_says_so() {
    let (prompts, log) = run(true, true);
    assert!(at(&prompts, "No test").is_some(), "{prompts:#?}");
    assert!(log.contains("fix round 1: compact failed (agent_prompt_stalled"), "{log}");
}
