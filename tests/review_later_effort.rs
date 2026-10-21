//! The reviewer's effort from a task's second review round on (reviewer.laterEffort): the first round keeps
//! reviewer.effort, later ones take the later effort, and each review says its effort in the task's runner.log.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

fn failing() -> Value { json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No test.", "findings": [{ "id": "F1", "severity": "major", "subject": "No test", "where": "f.txt", "proof": "read it" }] } }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
fn reviewer_starts(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("agent start") && c.contains("--kind codex ")).collect() }

fn two_rounds(settings: Value) -> (Vec<String>, String) {
    let mut s = json!({ "worker.agent": "claude", "reviewer.agent": "codex", "reviewer.effort": "high", "review.when": "always" });
    for (k, v) in settings.as_object().cloned().unwrap_or_default() { s[k] = v; }
    let w = world_with(Opts { repo: true, settings: s, ..Default::default() });
    w.script(json!({ "worker": [step(1), step(2)], "reviewer": [failing(), passing()] }));
    let id = w.add("Two rounds");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    (reviewer_starts(&w), std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn the_second_review_round_runs_at_the_later_effort_and_each_round_says_its_effort() {
    let (starts, log) = two_rounds(json!({ "reviewer.laterEffort": "medium" }));
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!(starts[0].contains("model_reasoning_effort=\"high\""), "round 1 keeps reviewer.effort: {}", starts[0]);
    assert!(starts[1].contains("model_reasoning_effort=\"medium\"") && !starts[1].contains("\"high\""), "round 2: {}", starts[1]);
    assert!(log.contains("review 1: effort high"), "{log}");
    assert!(log.contains("review 2: effort medium"), "{log}");
}

#[test]
fn without_a_later_effort_every_round_keeps_the_reviewers_effort() {
    let (starts, log) = two_rounds(json!({}));
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!(starts.iter().all(|s| s.contains("model_reasoning_effort=\"high\"")), "{starts:?}");
    assert!(log.contains("review 1: effort high") && log.contains("review 2: effort high"), "{log}");
}
