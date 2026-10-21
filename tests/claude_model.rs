//! Claude's model by issue label (worker.claudeModelByLabel, reviewer.claudeModelByLabel): chore, docs and research
//! run their worker, and their Claude review, on Sonnet; faithful and bug stay on the default; gh failing to read the
//! labels leaves the default, and runner.log says why.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing";
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
fn sonnet() -> Value { json!(["kind:chore=sonnet", "kind:docs=sonnet", "kind:research=sonnet"]) }
fn settings() -> Value { json!({ "worker.agent": "claude", "reviewer.agent": "claude", "worker.claudeModelByLabel": sonnet(), "reviewer.claudeModelByLabel": sonnet(), "review.when": "always" }) }
/// The task run to its review's pass; the worker's and the reviewer's start lines, and runner.log.
fn run(gh: Value) -> (String, String, String) {
    let w = world_with(Opts { repo: true, settings: settings(), ..Default::default() });
    w.gh_says(gh);
    w.script(json!({ "worker": [step(1)], "reviewer": [passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let starts: Vec<String> = calls(&w).into_iter().filter(|c| c.starts_with("agent start")).collect();
    let worker = starts.iter().find(|c| !c.contains("-r ") && !c.contains("-r-")).cloned().unwrap_or_default();
    let reviewer = starts.iter().find(|c| c.contains("-r ") || c.contains("-r-")).cloned().unwrap_or_default();
    (worker, reviewer, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn chore_docs_and_research_run_on_sonnet() {
    for kind in ["kind:chore", "kind:docs", "kind:research"] {
        let (worker, reviewer, log) = run(json!({ "labels": [kind, "p2"] }));
        assert!(worker.contains("--model sonnet"), "{kind}: {worker}");
        assert!(reviewer.contains("--model sonnet"), "{kind}: {reviewer}");
        assert!(log.contains(&format!("worker model: sonnet ({kind})")), "{log}");
        assert!(log.contains(&format!("review 1: reviewer claude sonnet ({kind})")), "{log}");
    }
}

#[test]
fn faithful_bug_and_unlabelled_run_on_the_default() {
    for labels in [json!(["kind:faithful"]), json!(["kind:bug", "p1"]), json!([])] {
        let (worker, reviewer, log) = run(json!({ "labels": labels }));
        assert!(!worker.contains("--model") && !reviewer.contains("--model"), "{labels}: {worker}\n{reviewer}");
        assert!(!log.contains("worker model:") && log.contains("review 1: reviewer claude\n"), "{log}");
        assert!(!log.contains("sonnet"), "{log}");
    }
}

#[test]
fn a_label_read_that_fails_gets_the_default_and_says_why() {
    let (worker, reviewer, log) = run(json!({ "labels": ["kind:docs"], "issueFail": true }));
    assert!(!worker.contains("--model") && !reviewer.contains("--model"), "{worker}\n{reviewer}");
    assert!(log.contains("worker model: default (gh failed on #12: HTTP 502)"), "{log}");
    assert!(log.contains("review 1: reviewer claude (default model: gh failed on #12: HTTP 502)"), "{log}");
}
