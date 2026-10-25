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
fn finished_by(agent: &str, script: &str, steps: Value) -> (String, Vec<String>, String) { reviewed_by(agent, script, steps, json!([passing()])) }
fn reviewed_by(agent: &str, script: &str, steps: Value, reviews: Value) -> (String, Vec<String>, String) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "worker.agent": agent, "review.when": "always", "pr.bodyNeeds": ["Checked:"], "pr.readyAlso": "Re-read each added sentence." }), ..Default::default() });
    let id = w.add("Fix the thing");
    let mut steps = steps;
    steps[0]["files"]["tools/pr_ready.sh"] = json!(script);
    // A note the turn writes itself, before it ends, as a real agent does (the stand-in writes `note` after the end).
    for s in steps.as_array_mut().unwrap() {
        if let Some(n) = s.get("inTurnNote").cloned() { s["files"][sw::store::files::dir_of(&id).join("closing-note.md").display().to_string()] = n; }
    }
    w.script(json!({ "worker": steps, "reviewer": reviews }));
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
    assert!(told[1].contains("pr_ready: the PR body has no \"Claims checked:\" line"), "{}", told[1]);
    // the warn goes with the failure, as advisory
    assert!(log.contains("pr_ready: 1 warn line sent"), "{log}");
    assert!(told[1].contains("Advisory (not blocking): fix or say in the body why not.\npr_ready warn: a.rs:3: names an address"), "{}", told[1]);
    assert!(told[1].contains("with a new commit. Re-read each added sentence. Then end your turn"), "{}", told[1]);
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
    // the Coordinator hears of it, once
    assert_eq!(log.matches("pr_ready: could not run, coordinator told").count(), 1, "{log}");
}

#[test]
fn a_fix_rounds_closing_note_is_the_body_before_the_next_review() {
    // FL-424, FL-431: the fixed closing-note.md never reached the pull request, so review 2 failed on the old body
    let failing = json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No.", "findings": [{ "id": "F1", "severity": "major", "subject": "The body has no Shots line", "where": "PR body" }] } });
    let (log, _, gh) = reviewed_by("claude", SCRIPT, json!([
        { "files": { "f.txt": "v1\n", "claims.ok": "\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "inTurnNote": "Fixed it.\n\nChecked: passes.\n\nShots: none, no screen.", "ends": "claude-finished-made", "hook": true },
    ]), json!([failing, passing()]));
    let synced = log.find("pr body: synced from the closing note").unwrap_or_else(|| panic!("{log}"));
    let second = log.find("review 2").unwrap_or_else(|| panic!("{log}"));
    assert!(log.find("review 1").unwrap() < synced && synced < second, "{log}");
    assert_eq!(log.matches("pr_ready: ok").count(), 2, "before each review: {log}");
    assert!(gh.contains("pr edit 7 --body Fixed it.\n\nChecked: passes.\n\nShots: none, no screen.\n"), "{gh}");
}

#[test]
fn a_round_with_no_new_note_leaves_the_body_alone() {
    let failing = json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No.", "findings": [{ "id": "F1", "severity": "major", "subject": "A typo", "where": "f.txt" }] } });
    let (log, _, gh) = reviewed_by("pi", SCRIPT, json!([
        { "files": { "f.txt": "v1\n", "claims.ok": "\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "files": { "f.txt": "v2\n" }, "commit": "typo" },
    ]), json!([failing, passing()]));
    assert!(log.contains("review 2") && !log.contains("pr body: synced"), "{log}");
    assert!(!log.contains("left alone"), "no note, nothing said: {log}");
    assert!(!gh.contains("pr edit 7 --body"), "{gh}");
}

#[test]
fn a_fix_rounds_note_that_is_no_body_leaves_the_body_alone() {
    let failing = json!({ "verdict": { "meets": false, "gate": "major", "pass": false, "text": "No.", "findings": [{ "id": "F1", "severity": "major", "subject": "A typo", "where": "f.txt" }] } });
    let (log, _, gh) = reviewed_by("claude", SCRIPT, json!([
        { "files": { "f.txt": "v1\n", "claims.ok": "\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "files": { "f.txt": "v2\n" }, "commit": "typo", "inTurnNote": "Round 2: fixed the typo.", "ends": "claude-finished-made", "hook": true },
    ]), json!([failing, passing()]));
    assert!(log.contains("review 2") && !log.contains("pr body: synced") && !log.contains("body refused"), "{log}");
    assert_eq!(log.matches("pr body: note is not a whole body, left alone").count(), 1, "{log}");
    assert!(!gh.contains("pr edit 7 --body"), "{gh}");
}

#[test]
fn warns_alone_are_sent_once_and_then_the_review_goes_on() {
    // FL-464: a warn the board dropped was what review 1 failed on
    let warn = "#!/usr/bin/env bash\necho 'pr_ready warn: a.rs:3: names an address with no [observed] in its block'\nexit 0\n";
    let (log, told, _) = finished(warn, json!([
        { "files": { "f.txt": "v1\n" }, "commit": "v1", "note": "Fixed it.\n\nChecked: passes." },
        { "files": { "f.txt": "v2\n" }, "commit": "v2", "note": "Fixed it.\n\nChecked: passes, a.rs:3 left: a quote." },
    ]));
    let sent = log.find("pr_ready: 1 warn line sent").unwrap_or_else(|| panic!("{log}"));
    let ok = log.find("pr_ready: ok").unwrap_or_else(|| panic!("{log}"));
    assert!(log.find("review 1").map(|r| sent < ok && ok < r).unwrap_or(false), "{log}");
    assert_eq!(log.matches("warn line").count(), 1, "{log}");
    assert_eq!(told.len(), 2, "{told:?}");
    assert!(told[1].contains("It passed, with warnings:\n\nAdvisory (not blocking): fix or say in the body why not.\npr_ready warn: a.rs:3") && told[1].contains("the review follows"), "{}", told[1]);
}
