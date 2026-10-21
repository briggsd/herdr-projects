//! Issue labels the pull request copies (pr.copyLabels): an issue with the label gives its pull request the label, once,
//! and says so in runner.log; without it, none; a label put on the issue mid-task reaches the pull request at the next
//! round; gh failing is said and never holds the task.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing";

fn failing() -> Value { json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No test.", "findings": [{ "id": "F1", "severity": "major", "subject": "No test", "where": "f.txt", "proof": "read it" }] } }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }

/// The task settled at the gate after `rounds` reviews (the last passing), with its runner.log and the label edits gh got.
fn ran(gh: Value, rounds: usize) -> (sw::store::tasks::Task, String, Vec<String>) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "worker.agent": "pi", "reviewer.agent": "codex", "pr.copyLabels": ["needs-human"], "review.when": "always" }), ..Default::default() });
    w.gh_says(gh);
    let mut verdicts = vec![failing(); rounds - 1]; verdicts.push(passing());
    w.script(json!({ "worker": (1..=rounds as i64).map(step).collect::<Vec<_>>(), "reviewer": verdicts }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let log = std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default();
    let edits = gh_log(&w).lines().filter(|l| l.starts_with("pr edit")).map(String::from).collect();
    (t, log, edits)
}

#[test]
fn an_issue_with_the_label_gives_its_pull_request_the_label_once() {
    let (t, log, edits) = ran(json!({ "labels": ["kind:faithful", "needs-human", "p2"] }), 1);
    assert_eq!(edits, vec!["pr edit https://github.com/o/r/pull/7 --add-label needs-human"], "{log}");
    assert_eq!(log.matches("pr labels: added needs-human from #12").count(), 1, "{log}");
    assert_eq!(t.arr("prLabels").len(), 1);
}

#[test]
fn without_the_label_the_pull_request_gets_none() {
    let (t, log, edits) = ran(json!({ "labels": ["kind:faithful", "p2"] }), 1);
    assert!(edits.is_empty() && !log.contains("pr labels"), "{edits:?} {log}");
    assert!(!t.b("prLabels"));
}

#[test]
fn a_label_put_on_the_issue_mid_task_reaches_the_pull_request_before_the_gate() {
    // The first two looks (the pull request opened, review 1 starting) see no label; it is on the issue from round 2.
    let (_, log, edits) = ran(json!({ "labels": ["p2"], "labelsLater": { "after": 2, "labels": ["p2", "needs-human"] } }), 2);
    assert_eq!(edits.len(), 1, "{edits:?}");
    let (r1, added, r2) = (log.find("review 1: reviewer").unwrap(), log.find("pr labels: added needs-human from #12").expect(&log), log.find("review 2: reviewer").unwrap());
    assert!(r1 < added && added < r2, "copied in round 2, before its review: {log}");
}

#[test]
fn gh_failing_is_said_and_never_holds_the_task() {
    let (_, log, _) = ran(json!({ "labels": ["needs-human"], "editFail": true }), 1);
    assert!(log.contains("pr labels: could not add needs-human from #12: HTTP 422: could not add label"), "{log}");
    let (_, log, edits) = ran(json!({ "labels": ["needs-human"], "issueFail": true }), 1);
    assert!(log.contains("pr labels: gh failed on #12: HTTP 502") && edits.is_empty(), "{log}");
}
