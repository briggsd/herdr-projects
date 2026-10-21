//! Who reviews a task by its issue's labels (`reviewer.byLabel`): a task whose GitHub issue has one of those labels is
//! reviewed by the first usable agent on `reviewer.fallback`, every round, on that agent's own effort. The issue is the
//! one the task's description names (finish/closes.rs); its labels are asked of gh when the review starts. With no
//! issue, no such label, or gh failing, `reviewer.agent` reviews as before, and the reason is said. From a task's second
//! round on, `reviewer.laterAgent` reviews whatever the labels (`fallback`: the first usable agent on the list; `same`:
//! no change), unless a label already routed it. Between the two, `reviewer.codexPace` labels pace Codex's weekly
//! allowance (finish/review_pace.rs). The issue's labels are asked of gh once per review.
use super::hand_over::usable;
use crate::json::*;
use crate::store::handovers::next_fallback;
use crate::store::settings::values::list_of;
use std::path::Path;

/// The routed reviewer, if any, and why (for the runner.log line); `why` is "" when no labels are set.
pub struct Route { pub agent: Option<String>, pub why: String }

pub fn route(t: &Obj, ch: &Obj, root: &Path, round: i64) -> Route {
    let has = issue_labels(t, ch, root);
    let r = by_label(ch, &has);
    if r.agent.is_some() { return r; }
    if let Some(p) = has.as_ref().ok().and_then(|(_, l)| super::review_pace::pace(ch, l)) { return p; }
    if round < 2 { return r; }
    let later = ch.t("reviewer.laterAgent");
    if later.is_empty() || later == "same" { return r; }
    let access = ch.s("reviewer.access");
    let a = if later == "fallback" { next_fallback(&list_of(ch.at("reviewer.fallback")), ch.s("reviewer.agent"), &[], |a| usable(a, access)).0 } else if usable(&later, access).is_empty() { later.clone() } else { String::new() };
    if a.is_empty() { return Route { agent: None, why: format!("round 2+, but no usable {}", if later == "fallback" { "reviewer.fallback agent".to_string() } else { later }) }; }
    if a == ch.s("reviewer.agent") { return r; }
    Route { agent: Some(a), why: "round 2+".into() }
}

/// The issue's number and labels, asked of gh only when a label setting wants them; Err: why not ("" for not wanted).
fn issue_labels(t: &Obj, ch: &Obj, root: &Path) -> Result<(u64, Vec<String>), String> {
    let set = |k: &str| list_of(ch.at(k)).into_iter().any(|l| l != "none");
    if !set("reviewer.byLabel") && !set("reviewer.codexPace") { return Err(String::new()); }
    let Some(n) = super::closes::issue_of(t, root) else { return Err("no issue".into()) };
    let r = crate::flow::git::gh(&["issue", "view", &n.to_string(), "--json", "labels", "-q", ".labels[].name"], root, 30000);
    if r.status != 0 { return Err(format!("gh failed on #{n}: {}", r.stderr.lines().next().unwrap_or("").trim())); }
    Ok((n, r.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect()))
}

fn by_label(ch: &Obj, issue: &Result<(u64, Vec<String>), String>) -> Route {
    let keep = |why: String| Route { agent: None, why };
    let labels: Vec<String> = list_of(ch.at("reviewer.byLabel")).into_iter().filter(|l| l != "none").collect();
    if labels.is_empty() { return keep(String::new()); }
    let (n, has) = match issue { Ok((n, h)) => (*n, h.iter().map(String::as_str).collect::<Vec<&str>>()), Err(why) => return keep(why.clone()) };
    let Some(hit) = has.iter().find(|l| labels.iter().any(|x| x == *l)) else {
        let kinds: Vec<&str> = has.iter().copied().filter(|l| l.starts_with("kind:")).collect();
        return keep(if kinds.is_empty() { format!("#{n} has no kind label") } else { kinds.join(", ") });
    };
    let (next, _) = next_fallback(&list_of(ch.at("reviewer.fallback")), ch.s("reviewer.agent"), &[], |a| usable(a, ch.s("reviewer.access")));
    if next.is_empty() { return keep(format!("{hit}, but no usable reviewer.fallback agent")); }
    Route { agent: Some(next), why: hit.to_string() }
}
