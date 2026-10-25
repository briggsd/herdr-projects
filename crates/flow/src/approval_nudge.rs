//! A task left waiting for approval: once it has waited `FIRST` with no merge intent, the Coordinator is told again
//! (the task, its pull request, its review's verdict line), and then every `AGAIN` it still waits. A request it missed
//! while busy or compacting otherwise left a passed task waiting for hours (fl-friction, 2026-10-25: FL-429, 5 h).
//! runner.log says `approval: re-sent to coordinator (N min waiting)`.
use crate::clock::now;
use crate::finish::shared::runner_log;
use crate::json::*;
use crate::notify::event_of;
use crate::store::keys::{repo_keys, tag_of};
use crate::store::reviews::review_history;
use crate::store::tasks::{update, Change, Task};
use std::path::Path;

const FIRST: i64 = 30 * 60000;
const AGAIN: i64 = 60 * 60000;
fn ms(name: &str, or: i64) -> i64 { std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(or) }

/// Since when the task has waited for approval, when it is one the Coordinator should hear about again now.
pub fn due_since(t: &Task, at: i64) -> Option<i64> {
    if event_of(t) != "ready to approve" || t.b("mergeIntent") || t.b("autoMerge") { return None; }
    let since = t.at("notice").i("at");
    if since == 0 || at - since < ms("HERDR_PROJECTS_APPROVAL_NUDGE_MS", FIRST) { return None; }
    let last = t.at("approvalNudge").i("at");
    if last >= since && at - last < ms("HERDR_PROJECTS_APPROVAL_AGAIN_MS", AGAIN) { return None; }
    Some(since)
}
/// What the Coordinator is told.
pub fn words(t: &Task, mins: i64) -> String {
    let tag = { let g = tag_of(t.s("id"), &repo_keys()); if g.is_empty() { t.t("id") } else { g } };
    let pr = t.s("pr");
    let verdict = review_history(t).last().map(|r| format!(" Review {}: {}", r.t("round"), r.t("text").lines().next().unwrap_or("").chars().take(300).collect::<String>())).unwrap_or_default();
    format!("[board] {tag} ({}) has waited {mins} min for approval: {}{}.{verdict} Approve it (board approve {}), or say why it waits.",
        t.t("id"), t.t("title"), if pr.is_empty() { String::new() } else { format!(", {pr}") }, t.t("id"))
}
/// The housekeeping pass, for the tasks of one herdr session.
pub fn nudge_pass(tasks: &[Task], socket: &str) {
    let at = now();
    for t in tasks.iter().filter(|t| t.at("link").s("socket") == socket) {
        let Some(since) = due_since(t, at) else { continue };
        let mins = (at - since) / 60000;
        let root = t.s("root");
        if crate::coordinator::requests::send(Path::new(if root.is_empty() { "." } else { root }), &words(t, mins), false, "", socket).is_err() { continue; }
        let _ = update(t.s("id"), |x| { x.put("approvalNudge", json!({ "at": at, "since": since })); Ok(Change::Write) });
        runner_log(t.s("id"), &format!("approval: re-sent to coordinator ({mins} min waiting)"));
    }
}
