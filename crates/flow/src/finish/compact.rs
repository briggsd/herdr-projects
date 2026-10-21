//! A fix round starts on a compacted conversation (`worker.compactBeforeFix`): before the findings go to a working
//! Claude, it is told `/compact` with what to keep, and the board waits until herdr sees it at rest again. The compact
//! is the board's own turn: herdr's working and done for it are not a turn of yours (expectedWake), its completion is
//! marked handled, and a turn-end report it made is dropped, so the fix turn's end is the one judged. A compact that
//! fails or stalls is said in runner.log, and the findings go anyway.
use super::shared::runner_log;
use crate::clock::now;
use crate::err::Res;
use crate::json::*;
use crate::runner::Guard;
use crate::store::marks::pending_events;
use crate::store::settings::lookup::work;
use crate::store::tasks::Task;

pub const KEEP: &str = "Keep: the issue number and acceptance list, the branch and files changed, addresses traced and what each showed, the last check.sh tree and result, and open questions. Drop: file dumps, listing excerpts already summarised, and finished searches.";
const WAIT_MS: u64 = 600_000;

/// Fix round `round` of task `t` is about to start: compact its worker first, when the setting asks and it is Claude.
pub fn before_fix(id: &str, g: &mut Guard, t: &Task, round: i64) -> Res<()> {
    if work(Some(t)).at("worker.compactBeforeFix") != &json!(true) || t.s("agent") != "claude" { return Ok(()); }
    let api = crate::flow::task::h(t);
    let pane = t.at("link").t("pane");
    if api.agent_get(&pane).ok().flatten().is_none() { runner_log(id, &format!("fix round {round}: not compacted (its agent is not running)")); return Ok(()); }
    let before = t.at("usage").i("context");
    let started = now();
    g.set(|t| { t.put("expectedWake", json!({ "reason": "compact", "keepIntent": t.b("mergeIntent"), "at": started })); })?;
    let r = api.agent_prompt_wait(&pane, &format!("/compact {KEEP}"), WAIT_MS);
    let a = api.agent_get(&pane).ok().flatten();
    g.set(|t| {
        if let Some(a) = &a { let h = t.obj_mut("handled"); h.put("comp", a.at("completion_seq").clone()); h.put("seq", a.at("state_change_seq").clone()); }
        let events: Vec<Value> = pending_events(t).into_iter().filter(|e| !(e.s("role") == "agent" && e.i("at") >= started)).collect();
        t.put("events", events);
    })?;
    runner_log(id, &match r {
        Ok(_) if before > 0 => format!("fix round {round}: compacted (before {before} tokens)"),
        Ok(_) => format!("fix round {round}: compacted"),
        Err(e) => format!("fix round {round}: compact failed ({}: {}); the findings go anyway", e.code, e.message),
    });
    Ok(())
}
