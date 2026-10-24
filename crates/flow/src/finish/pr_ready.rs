//! Before review, the repo's own check of the pull request (`pr.readyBeforeReview`): `bash tools/pr_ready.sh <pr>` in
//! the task's worktree. It checks the body (its Closes, Checked and Claims checked lines), and only can once the pull
//! request exists, which the board opens itself after the agent's turn: run by the agent before that, its body checks
//! wait, so a missing line was first found by the reviewer, a whole review round for one line (fl-friction,
//! 2026-10-23: FLM-111, FLM-114). Its failing lines go back to the agent, at most `SENDS` times; the next closing note
//! is put into the body (finish/pipeline.rs) and the script runs again. A script that cannot run holds nothing.
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
/// Whether the failing lines went back to the agent: the review waits for its next turn.
pub fn sent_back(id: &str, g: &mut Guard, ch: &Obj, tree: &Path, url: &str, keep_intent: bool) -> Res<bool> {
    let pending = g.check()?.b("prReadyPending");
    let clear = |g: &mut Guard| -> Res<()> { if pending { g.set(|t| t.put("prReadyPending", false))?; } Ok(()) };
    if !ch.b("pr.readyBeforeReview") || !tree.join(SCRIPT).is_file() { clear(g)?; return Ok(false); }
    let pr = crate::flow::git::pr_number(url);
    crate::runner::user_env();
    let mut c = std::process::Command::new("bash");
    c.args([SCRIPT, &pr]).current_dir(tree).env("GH_PROMPT_DISABLED", "1").env("GIT_TERMINAL_PROMPT", "0");
    let end = crate::remote::run_ended(c, RUN_MS);
    let bad = failing(&end.stdout);
    let last = [end.stderr.as_str(), end.stdout.as_str()].iter().flat_map(|s| s.lines()).filter(|l| !l.trim().is_empty()).last().unwrap_or("").trim().to_string();
    match end.code {
        Some(0) => { runner_log(id, "pr_ready: ok"); clear(g)?; Ok(false) }
        Some(1) if !bad.is_empty() => {
            let sent = g.check()?.i("prReadySent");
            if sent >= SENDS {
                runner_log(id, &format!("pr_ready: {} lines still fail after {sent} fixes, so the review goes on", bad.len()));
                clear(g)?;
                return Ok(false);
            }
            runner_log(id, &format!("pr_ready: {} line{} sent", bad.len(), if bad.len() == 1 { "" } else { "s" }));
            let words = format!("Before the review, the board ran `bash {SCRIPT} {pr}` on your pull request, and it failed:\n\n{}\n\nFix what these lines name. The board puts your closing note into the pull request's body, so write your closing note again as the whole body with them fixed (in closing-note.md, if you keep it there); fix and commit anything else they name. Then end your turn. The board checks it again and then asks for the review.", bad.join("\n"));
            let keep = keep_intent || g.check()?.b("mergeIntent");
            crate::flow::send::send_own(id, &words, "prready", keep, obj(json!({ "prReadyPending": true, "prReadySent": sent + 1, "prReadySentAt": crate::clock::now() })), 0)?;
            Ok(true)
        }
        _ => {
            let why = if end.timed_out { format!("no end in {} s", RUN_MS / 1000) } else if !end.error.is_empty() { end.error.clone() } else { format!("exit {}: {}", end.code.unwrap_or(-1), last) };
            runner_log(id, &format!("pr_ready: could not run ({why}), so the review goes on"));
            clear(g)?;
            Ok(false)
        }
    }
}
