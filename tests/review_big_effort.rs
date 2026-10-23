//! The reviewer's effort on a big change (reviewer.bigFiles, reviewer.bigEffort): a first round over a change of more
//! files than that reviews at the big effort and says why in runner.log; a smaller change keeps reviewer.effort.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

fn wide() -> Value { json!({ "files": { "a.txt": "a\n", "b.txt": "b\n", "c.txt": "c\n" }, "commit": "three files", "note": "Did three." }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }

fn reviewed(work: Value) -> (String, String) {
    let w = world_with(Opts { repo: true, settings: json!({ "worker.agent": "claude", "reviewer.agent": "codex", "reviewer.effort": "high", "reviewer.bigFiles": 2, "reviewer.bigEffort": "medium", "review.when": "always" }), ..Default::default() });
    w.script(json!({ "worker": [work], "reviewer": [passing()] }));
    let id = w.add("Change some files");
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let start = calls(&w).into_iter().find(|c| c.starts_with("agent start") && c.contains("--kind codex ")).unwrap_or_default();
    (start, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn a_first_round_over_more_files_than_big_files_reviews_at_the_big_effort() {
    let (start, log) = reviewed(wide());
    assert!(start.contains("model_reasoning_effort=\"medium\"") && !start.contains("\"high\""), "{start}");
    assert!(log.contains("review 1: effort medium (3 files > 2)"), "{log}");
}

#[test]
fn a_smaller_change_keeps_the_reviewers_effort() {
    let (start, log) = reviewed(step(1));
    assert!(start.contains("model_reasoning_effort=\"high\""), "{start}");
    assert!(log.contains("review 1: effort high\n") || log.contains("review 1: effort high\r"), "{log}");
}
