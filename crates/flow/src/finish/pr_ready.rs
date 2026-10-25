//! Before review, the repo's own check of the pull request (`pr.readyBeforeReview`): `bash tools/pr_ready.sh <pr>` in
//! the task's worktree. It checks the body (its Closes, Checked and Claims checked lines), and only can once the pull
//! request exists, which the board opens itself after the agent's turn: run by the agent before that, its body checks
//! wait, so a missing line was first found by the reviewer, a whole review round for one line (fl-friction,
//! 2026-10-23: FLM-111, FLM-114). Its failing lines go back to the agent, at most `SENDS` times a round; the next
//! closing note is put into the body (finish/pipeline.rs) and the script runs again, before every review round. A script that cannot run holds nothing.
//! Its `pr_ready warn:` lines (an address with no tag in its block: what reviewers then fail on, fl-friction 2026-10-25,
//! FL-464) go with them as advisory, each once a round; warns alone are sent once a round and never hold the review.
use super::shared::runner_log;
use crate::err::Res;
use crate::json::*;
use crate::runner::Guard;
use std::path::Path;

const SCRIPT: &str = "tools/pr_ready.sh";
const SENDS: i64 = 2;
const RUN_MS: u64 = 180000;

/// The lines that fail: `pr_ready: …`, not `pr_ready warn: …`.
pub fn failing(out: &str) -> Vec<String> {
    out.lines().map(str::trim_end).filter(|l| l.starts_with("pr_ready: ")).map(String::from).collect()
}
/// The advisory lines: `pr_ready warn: …`.
pub fn warns(out: &str) -> Vec<String> {
    out.lines().map(str::trim_end).filter(|l| l.starts_with("pr_ready warn: ")).map(String::from).collect()
}
fn advisory(w: &[String]) -> String {
    if w.is_empty() { String::new() } else { format!("\n\nAdvisory (not blocking): fix or say in the body why not.\n{}", w.join("\n")) }
}
/// The repo's own line said with them (`pr.readyAlso`), with its leading space.
fn also(ch: &Obj) -> String {
    let a = ch.t("pr.readyAlso");
    if a.trim().is_empty() || a.trim() == "none" { String::new() } else { format!(" {}", a.trim()) }
}
/// Whether the failing lines went back to the agent: the review waits for its next turn.
pub fn sent_back(id: &str, g: &mut Guard, ch: &Obj, tree: &Path, url: &str, keep_intent: bool) -> Res<bool> {
    let pending = g.check()?.b("prReadyPending");
    // Done with this round: the next round's failures get their own sends.
    let clear = |g: &mut Guard| -> Res<()> {
        let t = g.check()?;
        if pending || t.i("prReadySent") > 0 || t.b("prReadyWarned") || t.b("prReadyWarnOnly") { g.set(|t| { t.put("prReadyPending", false); t.put("prReadySent", 0); t.put("prReadyWarned", json!([])); t.put("prReadyWarnOnly", false); })?; }
        Ok(())
    };
    if !ch.b("pr.readyBeforeReview") || !tree.join(SCRIPT).is_file() { clear(g)?; return Ok(false); }
    let pr = crate::flow::git::pr_number(url);
    crate::runner::user_env();
    let mut c = std::process::Command::new("bash");
    c.args([SCRIPT, &pr]).current_dir(tree).env("GH_PROMPT_DISABLED", "1").env("GIT_TERMINAL_PROMPT", "0");
    let end = crate::remote::run_ended(c, RUN_MS);
    let bad = failing(&end.stdout);
    // Warn lines not sent yet this round.
    let told: Vec<String> = g.check()?.arr("prReadyWarned").iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let new: Vec<String> = warns(&end.stdout).into_iter().filter(|w| !told.contains(w)).collect();
    let note = crate::flow::task::note_file(&g.check()?);
    let fix = format!("rewriting {} as the whole pull request body. The board copies that file into the pull request, so don't run gh pr edit and don't summarise it in your reply. Fix any commit lines with a new commit.{}", note.display(), also(ch));
    let said_warns = |id: &str| if !new.is_empty() { runner_log(id, &format!("pr_ready: {} warn line{} sent", new.len(), if new.len() == 1 { "" } else { "s" })); };
    let warned = { let mut w = told.clone(); w.extend(new.iter().cloned()); w };
    let last = [end.stderr.as_str(), end.stdout.as_str()].iter().flat_map(|s| s.lines()).filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim().to_string();
    match end.code {
        Some(0) if !new.is_empty() && !g.check()?.b("prReadyWarnOnly") => {
            said_warns(id);
            let words = format!("Before the review, the board ran `bash {SCRIPT} {pr}` on your pull request. It passed, with warnings:{}\n\nFix a body line by {fix} Then end your turn; the review follows.", advisory(&new));
            let keep = keep_intent || g.check()?.b("mergeIntent");
            crate::flow::send::send_own(id, &words, "prready", keep, obj(json!({ "prReadyPending": true, "prReadyWarnOnly": true, "prReadyWarned": warned, "prReadySentAt": crate::clock::now() })), 0)?;
            Ok(true)
        }
        Some(0) => { runner_log(id, "pr_ready: ok"); clear(g)?; Ok(false) }
        Some(1) if !bad.is_empty() => {
            let sent = g.check()?.i("prReadySent");
            if sent >= SENDS {
                runner_log(id, &format!("pr_ready: {} lines still fail after {sent} fixes, so the review goes on", bad.len()));
                clear(g)?;
                return Ok(false);
            }
            runner_log(id, &format!("pr_ready: {} line{} sent", bad.len(), if bad.len() == 1 { "" } else { "s" }));
            said_warns(id);
            let words = format!("Before the review, the board ran `bash {SCRIPT} {pr}` on your pull request, and it failed on the body or the commits:\n\n{}{}\n\nFix the body lines by {fix} Then end your turn; the board checks it again before the review.", bad.join("\n"), advisory(&new));
            let keep = keep_intent || g.check()?.b("mergeIntent");
            crate::flow::send::send_own(id, &words, "prready", keep, obj(json!({ "prReadyPending": true, "prReadySent": sent + 1, "prReadyWarned": warned, "prReadySentAt": crate::clock::now() })), 0)?;
            Ok(true)
        }
        _ => {
            let why = if end.timed_out { format!("no end in {} s", RUN_MS / 1000) } else if !end.error.is_empty() { end.error.clone() } else { format!("exit {}: {}", end.code.unwrap_or(-1), last) };
            runner_log(id, &format!("pr_ready: could not run ({why}), so the review goes on"));
            // The Coordinator hears of it, once a task: a script that cannot run on this machine checks nothing, and
            // until then only runner.log said so (fl-friction, 2026-10-25: FLM's macOS bash 3.2, FLM-135).
            let t = g.check()?;
            if !t.b("prReadyToldCoordinator") {
                let first = [end.stderr.as_str(), end.stdout.as_str()].iter().flat_map(|s| s.lines()).map(str::trim).filter(|l| !l.is_empty()).find(|l| l.to_lowercase().contains("error")).map(String::from).unwrap_or_else(|| why.clone());
                let tag = { let k = crate::store::keys::tag_of(id, &crate::store::keys::repo_keys()); if k.is_empty() { id.to_string() } else { k } };
                let root = t.s("root");
                let text = format!("[board] pr_ready could not run on {tag}: {first}");
                if crate::coordinator::requests::send(Path::new(if root.is_empty() { "." } else { root }), &text, false, "", t.at("link").s("socket")).is_ok() {
                    g.set(|t| { t.put("prReadyToldCoordinator", true); })?;
                    runner_log(id, "pr_ready: could not run, coordinator told");
                }
            }
            clear(g)?;
            Ok(false)
        }
    }
}
