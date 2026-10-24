//! The pull request opens from the agent's latest closing note, once that note is a pull request body
//! (`pr.bodyNeeds`, `pr.bodyRefuses`): a note without the words it needs, or with words that say it is not one yet (a
//! note written before the pull request existed, FL-306), is refused once. The agent is told to write its closing note
//! as the body and end its turn again; its next note opens the pull request, as it is. A last message that is not a
//! body, beside a closing-note.md from the same turn that is, gives way to the note.
use super::shared::runner_log;
use crate::err::Res;
use crate::json::*;
use crate::runner::Guard;
use crate::store::settings::values::list_of;
use crate::store::keys::{repo_keys, task_ref};
use crate::store::tasks::Task;
use std::path::Path;

fn words(ch: &Obj, key: &str) -> Vec<String> { list_of(ch.at(key)).into_iter().filter(|w| w != "none" && !w.trim().is_empty()).collect() }

/// The body: the turn's last message, or its closing-note.md when the message is not a body and the note is (the
/// message only says the note was written, FL-315). runner.log says `pr body: from closing-note.md`.
pub fn latest(id: &str, t: &Task, ch: &Obj, said: String) -> String {
    let note = t.t("noteSaid");
    if note.trim().is_empty() || note == said || words(ch, "pr.bodyNeeds").is_empty() && words(ch, "pr.bodyRefuses").is_empty() { return said; }
    let plain = |s: &str| { let mut u = t.clone(); u.put("prBodyAsked", false); refused(&u, ch, s) };
    if plain(&said).is_some() && plain(&note).is_none() { runner_log(id, "pr body: from closing-note.md"); return note; }
    said
}

/// The pull request's body: the latest closing note with the issue's plain closing line, which the agent's note may
/// have left out, and the task's own line. Err: why that note is not a body yet. `rewritten`: the agent was asked to
/// write the body again (finish/pr_ready.rs), so this turn's closing-note.md is it, even beside a last message that
/// reads like one ("I rewrote the PR body … **Checked:** …", FL-420).
pub fn compose(id: &str, t: &Task, ch: &Obj, root: &Path, text: &str, rewritten: bool) -> Result<String, String> {
    let said = if t.b("said") { t.t("said") } else { text.to_string() };
    let note = if rewritten { this_turns_note(t).unwrap_or_else(|| t.t("noteSaid")) } else { String::new() };
    let latest = if rewritten && !note.trim().is_empty() { if note != said { runner_log(id, "pr body: from closing-note.md"); } note } else { latest(id, t, ch, said) };
    if let Some(why) = refused(t, ch, &latest) { return Err(why); }
    let mut said = if latest.is_empty() { t.t("title") } else { latest };
    if let Some(n) = super::closes::issue_of(t, root) {
        if let Some(b) = super::closes::with_closes(&said, n) { said = b; runner_log(id, &format!("pr body: added Closes #{n}")); }
    }
    Ok(format!("{said}\n\nTask: {}", task_ref(id, &repo_keys())))
}

/// closing-note.md as written since pr_ready's lines went to the agent. Read from the file: the note and the turn's
/// end come as two events, and the one that ends the turn may not carry it.
fn this_turns_note(t: &Task) -> Option<String> {
    let f = crate::flow::task::note_file(t);
    let at = std::fs::metadata(&f).and_then(|m| m.modified()).ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as f64;
    if at <= t.f("prReadySentAt") { return None; }
    std::fs::read_to_string(&f).ok().map(|s| s.trim_end().to_string()).filter(|s| !s.trim().is_empty())
}

/// Why `note` is not a pull request body yet (`no Checked`, `says "no PR yet"`), or None to open it: also None once the
/// task was told already.
pub fn refused(t: &Task, ch: &Obj, note: &str) -> Option<String> {
    if t.b("prBodyAsked") { return None; }
    let lower = note.to_lowercase();
    let mut why: Vec<String> = words(ch, "pr.bodyNeeds").into_iter().filter(|w| !lower.contains(&w.to_lowercase())).map(|w| format!("no {}", w.trim_end_matches(':'))).collect();
    why.extend(words(ch, "pr.bodyRefuses").into_iter().filter(|w| lower.contains(&w.to_lowercase())).map(|w| format!("says \"{w}\"")));
    (!why.is_empty()).then(|| why.join(", "))
}

/// The agent is told, once, to write its closing note as the pull request's body.
pub fn ask(id: &str, g: &mut Guard, why: &str) -> Res<()> {
    runner_log(id, &format!("pr open: body refused ({why})"));
    let t = g.set(|t| { t.put("prBodyAsked", true); })?;
    let words = format!("The board opens the pull request from your closing note, and that note is not a pull request body yet ({why}). Write your closing note again as the pull request's body, the way your instructions describe it (Closes #n, Checked, Shots, Also noticed, Friction), then end your turn. Nothing else needs to change.");
    crate::flow::send::send_own(id, &words, "prbody", t.b("mergeIntent"), Obj::new(), 0)?;
    Ok(())
}
