//! The repo's own check of the pull request before review (pr.readyBeforeReview, tools/pr_ready.sh): its failing lines
//! go back to the agent before any review, and the next closing note is put into the body and checked again; a passing
//! one goes to review; a script that cannot run is said in runner.log and the review goes on.
use crate::common;
use common::*;
use serde_json::{json, Value};
use sw::json::*;

/// Fails with one line (and a warning, which does not count) until the branch has claims.ok.
const SCRIPT: &str = "#!/usr/bin/env bash\n[ -f claims.ok ] && exit 0\necho 'pr_ready warn: a.rs:3: names an address'\necho 'pr_ready: the PR body has no \"Claims checked:\" line'\nexit 1\n";
fn passing() -> Value { json!({ "verdict": { "meets": true, "findings": [], "text": "Looks right." } }) }

/// runner.log, what the worker was told, and gh's calls.
fn finished(script: &str, steps: Value) -> (String, Vec<String>, String) { finished_by("pi", script, steps) }
fn finished_by(agent: &str, script: &str, steps: Value) -> (String, Vec<String>, String) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "worker.agent": agent, "review.when": "always" }), ..Default::default() });
    let id = w.add("Fix the thing");
    let mut steps = steps;
    steps[0]["files"]["tools/pr_ready.sh"] = json!(script);
    // A note the turn writes itself, before it ends, as a real agent does (the stand-in writes `note` after the end).
    for s in steps.as_array_mut().unwrap() {
        if let Some(n) = s.get("inTurnNote").cloned() { s["files"][sw::store::files::dir_of(&id).join("closing-note.md").display().to_string()] = n; }
    }
    w.script(json!({ "worker": steps, "reviewer": [passing()] }));
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{} {}", t.s("msg"), t.at("failure"));
    (std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default(), prompts(&w, t.at("link").s("pane")), gh_log(&w))
}

#[test]
fn a_failing_body_goes_back_to_the_worker_before_any_review() {
    let (log, told, gh) = finished(SCRIPT, json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "files": { "claims.ok": "\n" }, "commit": "claims", "note": "Fixed it.\n\nChecked: passes.\n\nClaims checked: none new." },
    ]));
    let sent = log.find("pr_ready: 1 line sent").unwrap_or_else(|| panic!("{log}"));
    let ok = log.find("pr_ready: ok").unwrap_or_else(|| panic!("{log}"));
    assert!(log.find("review 1").map(|r| sent < r && ok < r).unwrap_or(false), "no review before it passed: {log}");
    assert!(log.contains("pr body: synced from the closing note"), "{log}");
    assert_eq!(told.len(), 2, "{told:?}");
    assert!(told[1].contains("pr_ready: the PR body has no \"Claims checked:\" line") && !told[1].contains("warn"), "{}", told[1]);
    assert!(told[1].contains("/closing-note.md as the whole pull request body") && told[1].contains("don't run gh pr edit and don't summarise it in your reply"), "{}", told[1]);
    assert!(gh.contains("pr edit 7 --body Fixed it.\n\nChecked: passes.\n\nClaims checked: none new."), "{gh}");
}

#[test]
fn the_rewritten_body_is_the_closing_note_not_a_last_message_that_reads_like_one() {
    // FL-420: the fix turn's last message said "**Checked:** …" about the body, and went into the body itself
    let (log, _, gh) = finished_by("claude", SCRIPT, json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "files": { "claims.ok": "\n" }, "commit": "claims", "inTurnNote": "Fixed it.\n\nChecked: passes.\n\nClaims checked: none new.", "ends": "claude-fixed-the-body", "hook": true },
    ]));
    assert!(gh.contains("pr edit 7 --body Fixed it.\n\nChecked: passes.\n\nClaims checked: none new.\n"), "{gh}");
    assert!(!gh.contains("--body I rewrote"), "{gh}");
    assert!(log.contains("pr_ready: ok"), "{log}");
}

#[test]
fn a_passing_body_goes_to_review() {
    let (log, told, gh) = finished(SCRIPT, json!([{ "files": { "f.txt": "v1\n", "claims.ok": "\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." }]));
    assert!(log.contains("pr_ready: ok") && log.contains("review 1") && !log.contains("pr body: synced"), "{log}");
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(!gh.contains("pr edit 7 --body"), "{gh}");
}

#[test]
fn a_script_that_cannot_run_is_said_and_the_review_goes_on() {
    let (log, told, _) = finished("#!/usr/bin/env bash\necho 'pr_ready: unknown option --x'\nexit 2\n", json!([{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." }]));
    assert!(log.contains("pr_ready: could not run (exit 2: pr_ready: unknown option --x), so the review goes on") && log.contains("review 1"), "{log}");
    assert_eq!(told.len(), 1, "{told:?}");
}
