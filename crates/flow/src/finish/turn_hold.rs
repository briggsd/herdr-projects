//! A turn that ended with the agent's own background jobs still running (a check, a build, a fixtures run) is held:
//! nothing is committed, merged or moved on. The board says so in runner.log ("turn end held: <pid> <command>"), tells
//! the agent once to wait for the job, and judges its next turn's end again. The program's own helpers (an MCP or a
//! language server, herdr/jobs.rs `pane_jobs`) do not count. After three holds in a row, or with a job running over an
//! hour, the turn goes on as before, and runner.log says that too: a stuck job never holds a task for good.
use super::shared::runner_log;
use crate::err::Res;
use crate::flow;
use crate::json::*;
use crate::runner::Guard;
use crate::store::repos::cwd_of;
use crate::store::tasks::Task;

const HOLDS: i64 = 3;
fn max_age_s() -> i64 { std::env::var("HERDR_PROJECTS_HOLD_MAX_S").ok().and_then(|v| v.parse().ok()).unwrap_or(3600) }

/// None: held, and the agent told to wait. Some(text): the turn goes on, with `said` (and what the held turns said
/// before it, which the review and the commit still need).
pub fn hold(id: &str, g: &mut Guard, t: &Task, said: &str, keep_intent: bool) -> Res<Option<String>> {
    let api = flow::task::h(t);
    let program = crate::agents::kinds::program(&crate::store::settings::values::agent_of(t.s("agent")).kind).to_string();
    let jobs = api.pane_jobs(t.at("link").s("pane"), &program);
    let earlier = t.t("heldSaid");
    let go = |g: &mut Guard| -> Res<Option<String>> {
        if t.b("turnHolds") || t.b("heldSaid") { g.set(|t| { t.del("turnHolds"); t.del("heldSaid"); })?; }
        Ok(Some(if earlier.is_empty() || said.contains(earlier.trim()) { said.to_string() } else if said.trim().is_empty() { earlier.clone() } else { format!("{}\n\n{said}", earlier.trim_end()) }))
    };
    if jobs.is_empty() { return go(g); }
    let n = t.i("turnHolds");
    let line = |j: &crate::herdr::jobs::Job| format!("{} {}", j.pid, j.command);
    let old = jobs.iter().find(|j| j.age.is_some_and(|a| a > max_age_s()));
    if n >= HOLDS || old.is_some() {
        let why = match old { Some(j) => format!("{} has run {} min", j.pid, j.age.unwrap_or(0) / 60), None => format!("held {n} times already") };
        runner_log(id, &format!("turn end not held: {why}; going on with {} still running", jobs.iter().map(line).collect::<Vec<_>>().join("; ")));
        return go(g);
    }
    for j in &jobs { runner_log(id, &format!("turn end held: {}", line(j))); }
    let j = &jobs[0];
    let own = cwd_of(t).join("tools").join("wait_for.sh").exists(); // the repo's own waiter, where it has one
    let wait = if own { format!("Wait for it with bash tools/wait_for.sh {} <seconds>, then end your turn.", j.pid) } else { format!("Wait for it to end (while kill -0 {} 2>/dev/null; do sleep 5; done), then end your turn.", j.pid) };
    let more = if jobs.len() > 1 { format!(" Also still running: {}.", jobs[1..].iter().map(|j| format!("{} (pid {})", j.command, j.pid)).collect::<Vec<_>>().join("; ")) } else { String::new() };
    let words = format!("A background job you started is still running: {} (pid {}). {wait}{more}", j.command, j.pid);
    let keep_said = if earlier.is_empty() { said.to_string() } else { earlier.clone() };
    let patch = obj(json!({ "turnHolds": n + 1, "heldSaid": keep_said }));
    if let Err(e) = flow::send::send_own(id, &words, "held", keep_intent, patch, 0) {
        // Nothing reached the agent: the turn goes on as before, rather than waiting on a prompt that never came.
        runner_log(id, &format!("turn end not held: the agent could not be told ({})", e.msg));
        return go(g);
    }
    Ok(None)
}
