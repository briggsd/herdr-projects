//! After a review: what came since the commit it looked at (sync::since_review). A passed review stands when nothing
//! wants a look (no commit at all, or merges of main that went through clean and apart); a later round is told to look
//! at only what came after, and at the head's own checks (`review.laterChecks`).
use crate::common;
use common::*;
use serde_json::json;
use std::path::Path;
use sw::finish::review_brief::{review_brief, Brief};
use sw::json::*;
use sw::store::reviews::review_history;

/// The rule itself, on plain git.
#[test]
fn since_review_lists_own_commits_and_the_merges_of_main_that_want_a_look() {
    let w = world_with(Opts::default());
    let dir = w.base.join("s"); std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display().to_string();
    let write = |f: &str, t: &str| std::fs::write(dir.join(f), t).unwrap();
    let commit = |m: &str| { git(&dir, &["add", "-A"]); git(&dir, &["commit", "-qm", m]); git(&dir, &["rev-parse", "HEAD"]) };
    let merge = |c: &str| { let _ = common::worktrees::git(&dir, &["merge", "-q", "--no-edit", c]); };
    let since = |r: &str, m: &str| sw::sync::since_review(&d, r, m);
    git(&dir, &["init", "-q", "-b", "main"]);
    write("a.txt", "1\n2\n3\n4\n5\n6\n"); write("b.txt", "one\n"); write("c.txt", "one\n"); commit("init");
    git(&dir, &["checkout", "-q", "-b", "task"]); write("a.txt", "TASK\n2\n3\n4\n5\n6\n");
    let reviewed = commit("task");
    // nothing since: kept
    let s = since(&reviewed, "");
    assert!(sw::sync::nothing_to_look_at(&s), "{s}"); assert_eq!(s.t("from"), reviewed);
    // main moves apart from the change's files: a clean merge, nothing to look at
    git(&dir, &["checkout", "-q", "main"]); write("b.txt", "main\n"); let apart = commit("apart");
    git(&dir, &["checkout", "-q", "task"]); merge(&apart);
    let s = since(&reviewed, &apart);
    assert!(sw::sync::nothing_to_look_at(&s), "{s}"); assert_eq!(s.arr("merges").len(), 1);
    // with no main to hold it against, a merge is the branch's own work, all it brought in
    let s = since(&reviewed, "");
    assert_eq!((s.arr("own").len(), s.arr("other").len()), (0, 1)); assert!(!sw::sync::nothing_to_look_at(&s));
    // main's side changes the change's own file elsewhere: git merges it by itself, so nothing to look at either
    git(&dir, &["checkout", "-q", "main"]); write("a.txt", "1\n2\n3\n4\n5\nMAIN\n"); let touching = commit("touching");
    git(&dir, &["checkout", "-q", "-B", "t2", &reviewed]); merge(&touching);
    let s = since(&reviewed, &touching);
    assert!(sw::sync::nothing_to_look_at(&s), "{s}"); assert_eq!(s.arr("merges")[0].b("look"), false);
    // a conflict resolved by hand
    git(&dir, &["checkout", "-q", "main"]); write("a.txt", "MAIN\n2\n3\n4\n5\nMAIN\n"); let conflicting = commit("conflicting");
    git(&dir, &["checkout", "-q", "-B", "t3", &reviewed]); merge(&conflicting); write("a.txt", "BOTH\n2\n3\n4\n5\nMAIN\n"); let m = commit("resolve");
    let s = since(&reviewed, &conflicting);
    assert!(!sw::sync::nothing_to_look_at(&s)); assert!(s.arr("own").is_empty());
    let mm = &s.arr("merges")[0];
    assert_eq!((mm.t("commit"), mm.at("conflicts").clone(), mm.at("extra").clone(), mm.b("look")), (m.clone(), json!(["a.txt"]), json!([]), true));
    // more than the conflicts needed
    git(&dir, &["checkout", "-q", "-B", "t4", &reviewed]); merge(&conflicting); write("a.txt", "BOTH\n2\n3\n4\n5\nMAIN\n"); write("c.txt", "sneaked in\n"); commit("evil");
    assert_eq!(since(&reviewed, &conflicting).arr("merges")[0].at("extra"), &json!(["c.txt"]));
    // a commit of its own on top
    write("c.txt", "more\n"); let own = commit("more");
    let s = since(&reviewed, &conflicting);
    assert_eq!(s.at("own"), &json!([own])); assert_eq!(s.arr("merges").len(), 1);
    // rebuilt on main: the reviewed commit is gone, so all of the change is reviewed
    git(&dir, &["checkout", "-q", "-B", "rebased", &apart]); write("a.txt", "AGAIN\n2\n3\n4\n5\n6\n"); commit("again");
    let s = since(&reviewed, &apart);
    assert_eq!(s.at("ok"), &json!(false)); assert!(s.s("why").contains("no longer holds the reviewed commit"));
    assert_eq!(since("", "").at("ok"), &json!(false));
}

/// What a later round is told: the range since the last review, each merge that wants a look, the head's own checks.
#[test]
fn a_later_round_is_told_to_review_only_what_came_since_and_the_checks_on_the_head() {
    let _w = world();
    let t = obj(json!({ "id": "proj/1", "n": 1, "title": "Do it", "description": "" }));
    let since = json!({ "ok": true, "from": "1111111111aaaa", "head": "2222222222bbbb", "own": ["3333"], "other": ["6666666666eeee"], "merges": [
        { "commit": "4444444444cccc", "conflicts": ["a.txt"], "extra": [], "look": true },
        { "commit": "5555555555dddd", "conflicts": [], "extra": [], "look": false }] });
    let change = json!({ "head": "2222222222bbbb", "range": "base...HEAD", "stat": "1 file changed", "since": since });
    let mut ch = Obj::new();
    ch.put("review.laterChecks", "- The last commit's message is the squash message.");
    let brief = review_brief(&t, &Brief { text: "Did it.", change: Some(&change), to_land: true, ch: &ch, earlier: Some(&[]), check: "" });
    assert!(brief.contains("`git diff base...HEAD`"), "the whole change is still named");
    assert!(brief.contains("Earlier rounds reviewed the branch up to 1111111111. Review only what came after it"), "{brief}");
    assert!(brief.contains("- the agent's own commit since then: `git log -p --first-parent --no-merges 1111111111aaaa..2222222222bbbb`"), "{brief}");
    assert!(brief.contains("- the merge of main 4444444444 (its conflicts were resolved by hand in a.txt): `git show --remerge-diff 4444444444cccc`"), "{brief}");
    assert!(brief.contains("- the merge 6666666666 of something other than main: all it brought in is new, `git diff 6666666666eeee^1 6666666666eeee`"), "{brief}");
    assert!(brief.contains("1 other merge of main went through clean: the check covered it."), "{brief}");
    assert!(brief.contains("Whatever the range, also check these on the commit under review: they live in the head, not in the diff.\n- The last commit's message is the squash message."), "{brief}");
    // nothing new since the last review
    let same = json!({ "head": "1111111111aaaa", "range": "base...HEAD", "stat": "1 file changed", "since": { "ok": true, "from": "1111111111aaaa", "head": "1111111111aaaa", "own": [], "merges": [] } });
    let b2 = review_brief(&t, &Brief { text: "Did it.", change: Some(&same), to_land: true, ch: &Obj::new(), earlier: Some(&[]), check: "" });
    assert!(b2.contains("The last review stood on 1111111111, and nothing that wants a look came after it. Check the earlier findings"), "{b2}");
    assert!(!b2.contains("Whatever the range"), "no checks named, none told");
    // a first review, or a rebased branch: all of it, as ever
    let first = review_brief(&t, &Brief { text: "Did it.", change: Some(&json!({ "head": "h", "range": "r", "stat": "" })), to_land: true, ch: &Obj::new(), earlier: None, check: "" });
    assert!(!first.contains("Review only what came after it") && !first.contains("The last review stood on"));
}

/// A comment on the pull request woke the agent, it answered without changing the code: the passed review stands.
#[test]
fn a_turn_on_pull_request_feedback_that_changed_no_code_keeps_the_passed_review() {
    let w = world_with(Opts { repo: true, origin: true, settings: json!({ "review.when": "always" }), repo_settings: Some(json!({ "check.command": "true" })), ..Default::default() });
    let id = to_approval(&w, "Commented on");
    let t = task(&id);
    assert_eq!(t.i("reviewRound"), 1); assert_eq!(t.at("review").at("pass"), &json!(true));
    let say = |login: &str, body: &str, n: i64| json!({ "id": format!("c{n}"), "author": { "login": login }, "body": body, "updatedAt": format!("2026-01-0{n}") });
    w.script(json!({ "worker": [{ "note": "Answered on the pull request; the code stays as it is." }] }));
    w.gh_says(json!({ "user": "elias", "view": { "state": "OPEN", "headRefOid": t.at("prHead"), "statusCheckRollup": [], "reviews": [], "author": { "login": "elias" }, "comments": [say("elias", "Why this name?", 1)] } }));
    sw::housekeeping::poll_prs();
    let mut u = settle(&id); for _ in 0..2 { u = settle(&id); }
    assert_eq!(u.s("stage"), "approval", "{}", u.at("failure"));
    assert_eq!(u.i("reviewRound"), 1, "no new review"); assert_eq!(review_history(&u).len(), 1);
    assert!(u.at("review").b("keptAt")); assert_eq!(u.at("review").at("kept").at("same"), &json!(true));
    assert!(u.s("msg").contains("No code changed since review 1 passed, tests pass, it still stands."), "{}", u.s("msg"));
    assert_eq!(head_of(&u), t.t("reviewedHead"));
}
fn head_of(t: &Obj) -> String { git(Path::new(t.s("worktree")), &["rev-parse", "HEAD"]) }
