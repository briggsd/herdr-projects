//! The pull request opens from the agent's latest closing note, once that note is a pull request body
//! (`pr.bodyNeeds`, `pr.bodyRefuses`): a note without the words it needs, or with words that say it is not one yet (a
//! note written before the pull request existed, FL-306), is refused once. The agent is told to write its closing note
//! as the body and end its turn again; its next note opens the pull request, as it is.
use super::shared::runner_log;
use crate::err::Res;
use crate::json::*;
use crate::runner::Guard;
use crate::store::settings::values::list_of;
use crate::store::tasks::Task;

fn words(ch: &Obj, key: &str) -> Vec<String> { list_of(ch.at(key)).into_iter().filter(|w| w != "none" && !w.trim().is_empty()).collect() }

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
