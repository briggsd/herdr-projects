//! Codex paced by its weekly allowance (reviewer.codexPace): a faithful, bug or refactor issue goes to Codex while the
//! allowance used is under the week gone by, to the fallback (Claude) otherwise; a reading over an hour old sends one
//! review to Codex to refresh it; docs, chore and research still go to Claude by label; a Claude review cut off still
//! falls back to Codex.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing";
fn failing() -> Value { json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No test.", "findings": [{ "id": "F1", "severity": "major", "subject": "No test", "where": "f.txt", "proof": "read it" }] } }) }
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }
/// FL's rule: docs, chore and research to Claude by label, faithful, bug and refactor paced, Claude from round 2 otherwise.
fn paced() -> Value { json!({ "worker.agent": "pi", "reviewer.agent": "codex", "reviewer.effort": "high", "reviewer.laterEffort": "medium", "reviewer.fallback": ["claude"], "reviewer.byLabel": ["kind:docs", "kind:chore", "kind:research"], "reviewer.codexPace": ["kind:faithful", "kind:bug", "kind:refactor"], "reviewer.laterAgent": "fallback", "review.when": "always" }) }
/// Codex's session file with one weekly reading: `used` percent, `elapsed` percent of the week, read `age_s` ago.
fn codex_says(w: &World, used: f64, elapsed: f64, age_s: i64) {
    let home = w.root.join("codex-home");
    let dir = home.join("sessions/2026/10/21");
    std::fs::create_dir_all(&dir).unwrap();
    let now = chrono::Utc::now().timestamp();
    let reset = now + ((1.0 - elapsed / 100.0) * 604800.0) as i64;
    let at = chrono::DateTime::from_timestamp(now - age_s, 0).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let line = json!({ "timestamp": at, "type": "event_msg", "payload": { "type": "token_count", "rate_limits": { "primary": { "used_percent": used, "window_minutes": 10080, "resets_at": reset }, "secondary": null } } });
    std::fs::write(dir.join("rollout-x.jsonl"), format!("{{\"timestamp\":\"{at}\",\"type\":\"session_meta\"}}\n{line}\n")).unwrap();
    unsafe { std::env::set_var("CODEX_HOME", &home) };
}
fn kinds(w: &World) -> Vec<String> { calls(w).into_iter().filter(|c| c.starts_with("agent start") && !c.contains("--kind pi ")).map(|c| c.split("--kind ").nth(1).unwrap_or("").split(' ').next().unwrap_or("").to_string()).collect() }
fn run(w: &World, labels: Value, reviewer: Value) -> String {
    w.gh_says(json!({ "labels": labels }));
    w.script(json!({ "worker": [step(1), step(2)], "reviewer": reviewer }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", ISSUE); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    unsafe { std::env::remove_var("CODEX_HOME") };
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default()
}
fn world() -> World { world_with(Opts { repo: true, settings: paced(), ..Default::default() }) }

#[test]
fn under_pace_codex_reviews_every_round_at_its_efforts() {
    for kind in ["kind:faithful", "kind:bug", "kind:refactor"] {
        let w = world(); codex_says(&w, 20.0, 50.0, 60);
        let log = run(&w, json!([kind, "p2"]), json!([failing(), passing()]));
        assert_eq!(kinds(&w), vec!["codex", "codex"], "{kind}: {log}");
        assert!(log.contains("review 1: reviewer codex (pace: used 20% < elapsed 50% + 5)") && log.contains("review 1: effort high"), "{log}");
        assert!(log.contains("review 2: reviewer codex (pace: used 20% < elapsed 50% + 5)") && log.contains("review 2: effort medium"), "laterEffort: {log}");
    }
}

#[test]
fn over_pace_claude_reviews() {
    let w = world(); codex_says(&w, 80.0, 50.0, 60);
    let log = run(&w, json!(["kind:faithful"]), json!([failing(), passing()]));
    assert_eq!(kinds(&w), vec!["claude", "claude"], "{log}");
    assert!(log.contains("review 1: reviewer claude (pace: used 80% >= elapsed 50% + 5)") && log.contains("review 1: effort default"), "{log}");
}

#[test]
fn a_stale_reading_sends_one_review_to_codex_and_the_next_is_paced_on_it() {
    let w = world(); codex_says(&w, 80.0, 50.0, 7200);
    let log = run(&w, json!(["kind:faithful"]), json!([failing(), passing()]));
    assert_eq!(kinds(&w), vec!["codex", "claude"], "{log}");
    assert!(log.contains("review 1: reviewer codex (pace: reading 120m old, one review to refresh it)"), "{log}");
    assert!(log.contains("review 2: reviewer claude (pace: used 80% >= elapsed 50% + 5)"), "one only: {log}");
}

#[test]
fn docs_chore_and_research_still_go_to_claude() {
    for kind in ["kind:docs", "kind:chore", "kind:research"] {
        let w = world(); codex_says(&w, 0.0, 90.0, 60);
        let log = run(&w, json!([kind]), json!([failing(), passing()]));
        assert_eq!(kinds(&w), vec!["claude", "claude"], "{kind}: {log}");
        assert!(log.contains(&format!("review 1: reviewer claude ({kind})")), "{log}");
    }
}

#[test]
fn a_paced_claude_review_cut_off_falls_back_to_codex() {
    let w = cut_world("30,30", paced()); codex_says(&w, 80.0, 50.0, 60);
    let log = run(&w, json!(["kind:faithful"]), json!([{ "ends": "claude-usage-limit-made" }, passing()]));
    assert_eq!(kinds(&w), vec!["claude", "codex"], "{log}");
    assert!(log.contains("review 1: reviewer claude (pace: used 80% >= elapsed 50% + 5)") && log.contains("codex, the routed review's last resort, reviews"), "{log}");
}
