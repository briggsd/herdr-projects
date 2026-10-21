//! The pull request's plain "Closes #n" line: put in when the agent's closing message left it out, for the issue the
//! task's description says it implements, and said in the task's runner.log; never for a task without one.
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;
use sw::store::tasks::{update, Change};

const ISSUE: &str = "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/12): Fix the thing\n\nSplit from #10.";

/// The body the board opened the pull request with, and the task's runner.log.
fn opened(description: &str, note: &str) -> (String, String) {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "never" }), ..Default::default() });
    w.script(json!({ "worker": [{ "files": { "f.txt": "v1\n" }, "commit": "v1", "note": note }] }));
    let id = w.add("Fix the thing");
    update(&id, |c| { c.put("description", description); Ok(Change::Write) }).unwrap();
    delegate(&id);
    let t = settle(&id);
    assert_eq!(t.s("stage"), "approval", "{}", t.at("failure"));
    let log = gh_log(&w);
    let call = log.split("pr create ").nth(1).unwrap_or_else(|| panic!("no pr create: {log}"));
    let body = call.split_once(" --body ").unwrap().1;
    let body = body.split(" --base ").next().unwrap();
    let body = body.split("\nTask: ").next().unwrap().trim_end().to_string();
    (body, std::fs::read_to_string(sw::store::files::dir_of(&id).join("runner.log")).unwrap_or_default())
}

#[test]
fn a_missing_closes_line_goes_in_after_the_leading_summary_bullets() {
    let (body, log) = opened(ISSUE, "- Fixed the thing.\n- Added a test.\n\n**Checked**\n- tools/check.sh passes.");
    assert_eq!(body, "- Fixed the thing.\n- Added a test.\n\nCloses #12\n\n**Checked**\n- tools/check.sh passes.");
    assert!(log.contains("pr body: added Closes #12"), "{log}");
}

#[test]
fn an_issue_named_only_inside_a_sentence_gets_the_line_at_the_top() {
    let (body, log) = opened(ISSUE, "This closes #12 by fixing the thing.\n\n- Added a test.");
    assert_eq!(body, "Closes #12\n\nThis closes #12 by fixing the thing.\n\n- Added a test.");
    assert_eq!(log.matches("pr body: added Closes #12").count(), 1, "{log}");
}

#[test]
fn a_body_with_the_line_already_is_left_as_it_is() {
    let (body, log) = opened(ISSUE, "- Fixed the thing.\n\nCloses #12  \n\n**Checked**\n- ok.");
    assert_eq!(body, "- Fixed the thing.\n\nCloses #12  \n\n**Checked**\n- ok.");
    assert!(!log.contains("pr body:"), "{log}");
}

#[test]
fn a_task_without_an_issue_gets_no_line() {
    for d in [
        "Fix the thing. Depends on #12; see PR #13.",
        "Implements the next stage of GitHub issue o/r#12 (https://github.com/o/r/issues/12): more",
        "Implements GitHub issue o/r#12 (https://github.com/o/r/pull/12): a pull request, not an issue",
        "Implements GitHub issue o/r#12 (https://github.com/o/r/issues/13): numbers that disagree",
    ] {
        let (body, log) = opened(d, "Fixed the thing.");
        assert_eq!(body, "Fixed the thing.", "{d}");
        assert!(!log.contains("pr body:"), "{d}: {log}");
    }
}
