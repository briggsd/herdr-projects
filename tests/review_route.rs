//! Who reviews by the issue's labels (reviewer.byLabel): a docs, chore or research issue goes to the first agent on
//! reviewer.fallback, every round; any other kind, no label, no issue or gh failing keeps reviewer.agent, and each
//! review says its reviewer and why in the task's runner.log.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing";

fn failing() -> Value { json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No test.", "findings": [{ "id": "F1", "severity": "major", "subject": "No test", "where": "f.txt", "proof": "read it" }] } }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }

/// Two review rounds; the kinds of the reviewers started, in order, and the runner.log.
fn reviewed(description: &str, gh: Value) -> (Vec<String>, String) {
    let w = world_with(Opts { repo: true, settings: json!({ "worker.agent": "pi", "reviewer.agent": "codex", "reviewer.effort": "high", "reviewer.laterEffort": "medium", "reviewer.fallback": ["claude"], "reviewer.byLabel": ["kind:docs", "kind:chore", "kind:research"], "review.when": "always" }), ..Default::default() });
    w.gh_says(gh);
    w.script(json!({ "worker": [step(1), step(2)], "reviewer": [failing(), passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", description); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    let kinds = calls(&w).into_iter().filter(|c| c.starts_with("agent start") && !c.contains("--kind pi ")).map(|c| c.split("--kind ").nth(1).unwrap_or("").split(' ').next().unwrap_or("").to_string()).collect();
    (kinds, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn docs_chore_and_research_issues_are_reviewed_by_the_fallback_agent_every_round() {
    for kind in ["kind:docs", "kind:chore", "kind:research"] {
        let (kinds, log) = reviewed(ISSUE, json!({ "labels": ["status:in-progress", kind, "area:hud"] }));
        assert_eq!(kinds, vec!["claude", "claude"], "{kind}");
        assert!(log.contains(&format!("review 1: reviewer claude ({kind})")) && log.contains(&format!("review 2: reviewer claude ({kind})")), "{log}");
        assert!(log.contains("review 1: effort default") && log.contains("review 2: effort default"), "its own effort: {log}");
    }
}

#[test]
fn a_faithful_issue_stays_on_the_reviewer_at_its_efforts() {
    let (kinds, log) = reviewed(ISSUE, json!({ "labels": ["kind:faithful"] }));
    assert_eq!(kinds, vec!["codex", "codex"]);
    assert!(log.contains("review 1: reviewer codex (kind:faithful)") && log.contains("review 1: effort high") && log.contains("review 2: effort medium"), "{log}");
}

#[test]
fn no_label_no_issue_or_gh_failing_keeps_the_reviewer_and_says_why() {
    let (kinds, log) = reviewed(ISSUE, json!({ "labels": [] }));
    assert_eq!(kinds, vec!["codex", "codex"]);
    assert!(log.contains("review 1: reviewer codex (#12 has no kind label)"), "{log}");
    let (kinds, log) = reviewed(ISSUE, json!({ "issueFail": true, "labels": ["kind:docs"] }));
    assert_eq!(kinds, vec!["codex", "codex"]);
    assert!(log.contains("review 1: reviewer codex (gh failed on #12: HTTP 502)"), "{log}");
    let (kinds, log) = reviewed("Fix the thing; docs only (#12).", json!({ "labels": ["kind:docs"] }));
    assert_eq!(kinds, vec!["codex", "codex"]);
    assert!(log.contains("review 1: reviewer codex (no issue)"), "{log}");
}

#[test]
fn a_routed_review_cut_off_with_nobody_left_on_the_list_goes_back_to_the_reviewer_and_says_so() {
    let w = cut_world("30,30", json!({ "worker.agent": "pi", "reviewer.agent": "codex", "reviewer.effort": "high", "reviewer.fallback": ["claude"], "reviewer.byLabel": ["kind:docs"], "review.when": "always" }));
    w.gh_says(json!({ "labels": ["kind:docs"] }));
    w.script(json!({ "worker": [step(1)], "reviewer": [{ "ends": "claude-usage-limit-made" }, passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "the review is not failed: {} {}", t.s("msg"), t.at("failure"));
    let kinds: Vec<String> = calls(&w).into_iter().filter(|c| c.starts_with("agent start") && !c.contains("--kind pi ")).map(|c| c.split("--kind ").nth(1).unwrap_or("").split(' ').next().unwrap_or("").to_string()).collect();
    assert_eq!(kinds, vec!["claude", "codex"]);
    let codex = calls(&w).into_iter().find(|c| c.starts_with("agent start") && c.contains("--kind codex ")).unwrap();
    assert!(codex.contains("model_reasoning_effort=\"high\""), "at the reviewer's own effort: {codex}");
    let log = std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default();
    assert!(log.contains("review 1: reviewer claude (kind:docs)") && log.contains("claude was cut off") && log.contains("codex, the routed review's last resort, reviews"), "{log}");
}

/// FL's quota rule: reviewer.laterAgent `fallback`, with bug and refactor routed by label too.
fn quota() -> Value { json!({ "worker.agent": "pi", "reviewer.agent": "codex", "reviewer.effort": "high", "reviewer.laterEffort": "medium", "reviewer.fallback": ["claude"], "reviewer.byLabel": ["kind:docs", "kind:chore", "kind:research", "kind:bug", "kind:refactor"], "reviewer.laterAgent": "fallback", "review.when": "always" }) }
fn reviewer_kinds(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("agent start") && !c.contains("--kind pi ")).map(|c| c.split("--kind ").nth(1).unwrap_or("").split(' ').next().unwrap_or("").to_string()).collect() }
fn later(gh: Value) -> (Vec<String>, String) {
    let w = world_with(Opts { repo: true, settings: quota(), ..Default::default() });
    w.gh_says(gh);
    w.script(json!({ "worker": [step(1), step(2)], "reviewer": [failing(), passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    (reviewer_kinds(&w), std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn with_later_agent_a_faithful_issue_has_codex_in_round_one_and_the_fallback_after() {
    let (kinds, log) = later(json!({ "labels": ["kind:faithful"] }));
    assert_eq!(kinds, vec!["codex", "claude"]);
    assert!(log.contains("review 1: reviewer codex (kind:faithful)") && log.contains("review 1: effort high"), "{log}");
    assert!(log.contains("review 2: reviewer claude (round 2+)") && log.contains("review 2: effort default"), "its own effort, not laterEffort: {log}");
}

#[test]
fn with_later_agent_a_bug_or_refactor_issue_is_the_fallback_s_from_round_one() {
    for kind in ["kind:bug", "kind:refactor"] {
        let (kinds, log) = later(json!({ "labels": [kind] }));
        assert_eq!(kinds, vec!["claude", "claude"], "{kind}");
        assert!(log.contains(&format!("review 1: reviewer claude ({kind})")) && log.contains(&format!("review 2: reviewer claude ({kind})")), "{log}");
    }
}

#[test]
fn with_later_agent_an_unlabelled_issue_has_codex_then_the_fallback() {
    let (kinds, log) = later(json!({ "labels": [] }));
    assert_eq!(kinds, vec!["codex", "claude"]);
    assert!(log.contains("review 1: reviewer codex (#12 has no kind label)") && log.contains("review 2: reviewer claude (round 2+)"), "{log}");
}

#[test]
fn a_round_two_review_cut_off_goes_to_codex_for_that_round_and_round_three_tries_the_fallback_again() {
    let w = cut_world("30,30", quota());
    w.gh_says(json!({ "labels": ["kind:faithful"] }));
    // codex r1 sends back; claude r2 is cut off, codex reviews r2 and sends back; claude reviews r3.
    w.script(json!({ "worker": [step(1), step(2), step(3)], "reviewer": [failing(), { "ends": "claude-usage-limit-made" }, failing(), passing()] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    assert_eq!(reviewer_kinds(&w), vec!["codex", "claude", "codex", "claude"]);
    let log = std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default();
    assert!(log.contains("review 2: reviewer claude (round 2+)") && log.contains("codex, the routed review's last resort, reviews") && log.contains("review 3: reviewer claude (round 2+)"), "{log}");
}
