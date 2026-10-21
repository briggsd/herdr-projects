//! What came after the commit a review looked at: the branch's own commits since then, and each merge of main with
//! what in it still wants a look. A review after a pass is kept when there is nothing to look at (finish/pipeline.rs);
//! a later round reviews only what is listed here (finish/review_brief.rs). The rule, per commit on the branch's own
//! (first-parent) line since the reviewed one:
//!   - a merge of main (two parents: the branch as it was, and a commit `target` holds) is looked at only when git
//!     could not merge it by itself (`conflicts`), or when it differs from what git makes of it beyond those (`extra`).
//!     A clean merge is never looked at, whatever files main's side changed: the check proves it;
//!   - any other merge (of another branch, or of main when `target` is not known) is the branch's own work, all of
//!     what it brought in (`other`);
//!   - anything else is the branch's own work (`own`).
//! `ok` is false when the reviewed commit is not on that line any more (a rebase): then all of the change is reviewed.
use crate::flow::git::head_of;
use crate::json::*;
use crate::store::repos::{git_full, git_out};
use crate::store::tasks::one_line;
use std::path::Path;

fn names(dir: &Path, args: &[&str]) -> crate::err::Res<Vec<String>> { Ok(git_out(dir, args)?.split('\0').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()) }
/// One merge of main: what git could not merge, and what differs beyond that.
fn merge_of_main(dir: &Path, c: &str, ours: &str, theirs: &str) -> crate::err::Res<Option<Value>> {
    let (status, out, _) = git_full(dir, &["merge-tree", "--write-tree", "--name-only", "--no-messages", "-z", ours, theirs], 60000);
    if status != 0 && status != 1 { return Ok(None); }
    let parts: Vec<String> = out.split('\0').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
    let auto = parts.first().cloned().unwrap_or_default();
    if auto.len() < 40 || !auto.bytes().all(|b| b.is_ascii_hexdigit()) { return Ok(None); }
    let conflicts: Vec<String> = parts[1..].to_vec();
    let extra: Vec<String> = names(dir, &["diff", "--name-only", "--no-renames", "-z", &auto, &format!("{c}^{{tree}}")])?.into_iter().filter(|f| !conflicts.contains(f)).collect();
    let look = !conflicts.is_empty() || !extra.is_empty();
    Ok(Some(json!({ "commit": c, "conflicts": conflicts, "extra": extra, "look": look })))
}
/// { ok, from, head, own: [commit], other: [commit], merges: [{ commit, conflicts, extra, look }], why }
pub fn since_review(tree: &str, reviewed: &str, target: &str) -> Value {
    let no = |why: String| json!({ "ok": false, "why": why, "own": [], "merges": [] });
    if tree.is_empty() || reviewed.is_empty() { return no("the reviewed commit is not known".into()); }
    let dir = Path::new(tree);
    let run = || -> crate::err::Res<Value> {
        let head = head_of(dir);
        if head.is_empty() { return Ok(no("the branch's commit is not known".into())); }
        if head == reviewed { return Ok(json!({ "ok": true, "from": reviewed, "head": head, "own": [], "merges": [] })); }
        if git_out(dir, &["merge-base", "--is-ancestor", reviewed, &head]).is_err() { return Ok(no("the branch no longer holds the reviewed commit".into())); }
        let chain: Vec<String> = git_out(dir, &["rev-list", "--first-parent", "--reverse", &format!("{reviewed}..{head}")])?.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        let (mut own, mut other, mut merges, mut prev) = (vec![], vec![], vec![], reviewed.to_string());
        for c in &chain {
            let parents: Vec<String> = git_out(dir, &["show", "-s", "--format=%P", c])?.split_whitespace().map(String::from).collect();
            let of_main = parents.len() == 2 && parents[0] == prev && !target.is_empty() && git_out(dir, &["merge-base", "--is-ancestor", &parents[1], target]).is_ok();
            match if of_main { merge_of_main(dir, c, &parents[0], &parents[1])? } else { None } {
                Some(m) => merges.push(m),
                None if parents.len() > 1 => other.push(json!(c)), // all it brought in is the agent's to answer for
                None => own.push(json!(c)),
            }
            prev = c.clone();
        }
        if chain.is_empty() || prev != head { return Ok(no("the reviewed commit is not on the branch's own line".into())); }
        Ok(json!({ "ok": true, "from": reviewed, "head": head, "own": own, "other": other, "merges": merges }))
    };
    run().unwrap_or_else(|e| no(format!("git could not say ({})", one_line(&e.msg, 160))))
}
/// Nothing since the review wants a look: no own commit, and every merge of main went through clean.
pub fn nothing_to_look_at(s: &Value) -> bool { s.b("ok") && s.arr("own").is_empty() && s.arr("other").is_empty() && s.arr("merges").iter().all(|m| !m.b("look")) }
