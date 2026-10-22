//! An approval is for the commits you saw: when a task's branch gains a commit of its own after you approved it (a
//! fix for a check that failed on the branch brought up to date at the merge), the approval is dropped and the task
//! waits at "Approve to merge?" again. The board's own merges of main keep it, conflict resolutions in those merges
//! included: only commits that are not merges and not on main count. Said in runner.log ("approval dropped: ...").
use crate::json::*;
use crate::store::repos::git_out;
use crate::store::tasks::Task;
use std::path::Path;

/// The commits of its own (`<short sha> <subject>`) the branch gained between the commit you approved and `head`; empty
/// when there are none, the approval was the board's own (`merge.by`), or it was given before the board kept its commit.
pub fn own_since(t: &Task, folder: &Path, root: &Path, head: &str) -> Vec<String> {
    let mi = t.at("mergeIntent");
    let approved = mi.t("approvedHead");
    if approved.is_empty() || mi.b("auto") || head.is_empty() || approved == head { return vec![]; }
    let b = crate::sync::named_base(root, t).unwrap_or_else(|| crate::sync::main_branch(root, t));
    let main = [format!("refs/remotes/origin/{b}"), format!("refs/heads/{b}")].into_iter().find(|r| git_out(folder, &["rev-parse", "--verify", "-q", r]).is_ok()).unwrap_or_default();
    let range = format!("{approved}..{head}");
    let mut args = vec!["log", "--no-merges", "--format=%h %s", range.as_str()];
    if !main.is_empty() { args.extend(["--not", main.as_str()]); }
    git_out(folder, &args).map(|s| s.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()).unwrap_or_default()
}
