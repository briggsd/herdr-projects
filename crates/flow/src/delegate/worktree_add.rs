//! Making a task's worktree: one `herdr worktree create` at a time per repo, and tried again when git's config was
//! locked. Every worktree of a repo shares its .git/config, and git writes a new branch's upstream there. Starts that
//! land together (the spend throttle lets several go at once) took that lock from each other: "could not lock config
//! file .git/config: File exists" left an empty branch and the task in error (fl-coord, 2026-10-22, FL-331). A git
//! command in another worktree can hold it too, so a lock is waited out (2 s, then 4) before the start fails.
use crate::err::Res;
use crate::flow;
use crate::herdr::Herdr;
use crate::json::*;
use crate::store::files::with_lock_wait;
use crate::store::repos::{git_out, primary_root};
use std::path::Path;

const TRIES: u64 = 3;

/// Whether git said a lock file was in the way: .git/config's, or any other `.lock` another git command held.
pub fn locked(said: &str) -> bool {
    let s = said.to_lowercase();
    s.contains("could not lock config file") || (s.contains(".lock") && s.contains("file exists"))
}
fn pause(n: u64) {
    let step = std::env::var("HERDR_PROJECTS_FETCH_PAUSE_MS").ok().and_then(|v| v.parse::<u64>().ok()).unwrap_or(2000);
    if step > 0 { crate::clock::sleep_ms(step * n); }
}
/// `herdr worktree create` for `branch` from `from`, as delegating asks for it. `said` writes a line to the task's
/// runner.log.
pub fn create(api: &Herdr, root: &Path, branch: &str, from: &str, label: &str, said: &dyn Fn(&str)) -> Res<Value> {
    with_lock_wait(&format!("worktree:{}", primary_root(root).display()), 180000, || {
        let mut n = 1;
        loop {
            match flow::spaces::opening_worktree(api, root, || api.worktree_create(&root.display().to_string(), branch, from, label)) {
                Ok(v) => return Ok(v),
                Err(e) if n < TRIES && locked(&e.message) => {
                    let gone = drop_empty(root, branch, from);
                    said(&format!("worktree: a git lock was in the way ({}), trying again{}",
                        e.message.lines().find(|l| locked(l)).unwrap_or("").trim().trim_start_matches("error: "), if gone { "; its empty branch is deleted" } else { "" }));
                    pause(n);
                    n += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }
    })
}
/// The branch a create that failed left behind: deleted when it is still at its base and no worktree has it out.
fn drop_empty(root: &Path, branch: &str, from: &str) -> bool {
    let Ok(tip) = git_out(root, &["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]) else { return false };
    if git_out(root, &["rev-parse", &format!("{from}^{{commit}}")]).ok().as_deref() != Some(tip.as_str()) { return false; }
    let out = git_out(root, &["worktree", "list", "--porcelain"]).map(|l| l.lines().any(|x| x == format!("branch refs/heads/{branch}"))).unwrap_or(true);
    !out && git_out(root, &["branch", "-D", branch]).is_ok()
}
