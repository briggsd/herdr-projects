//! Who reviews a task by its issue's labels (`reviewer.byLabel`): a task whose GitHub issue has one of those labels is
//! reviewed by the first usable agent on `reviewer.fallback`, every round, on that agent's own effort. The issue is the
//! one the task's description names (finish/closes.rs); its labels are asked of gh when the review starts. With no
//! issue, no such label, or gh failing, `reviewer.agent` reviews as before, and the reason is said.
use super::hand_over::usable;
use crate::json::*;
use crate::store::handovers::next_fallback;
use crate::store::settings::values::list_of;
use std::path::Path;

/// The routed reviewer, if any, and why (for the runner.log line); `why` is "" when no labels are set.
pub struct Route { pub agent: Option<String>, pub why: String }

pub fn route(t: &Obj, ch: &Obj, root: &Path) -> Route {
    let keep = |why: String| Route { agent: None, why };
    let labels: Vec<String> = list_of(ch.at("reviewer.byLabel")).into_iter().filter(|l| l != "none").collect();
    if labels.is_empty() { return keep(String::new()); }
    let Some(n) = super::closes::issue_of(t, root) else { return keep("no issue".into()) };
    let r = crate::flow::git::gh(&["issue", "view", &n.to_string(), "--json", "labels", "-q", ".labels[].name"], root, 30000);
    if r.status != 0 { return keep(format!("gh failed on #{n}: {}", r.stderr.lines().next().unwrap_or("").trim())); }
    let has: Vec<&str> = r.stdout.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let Some(hit) = has.iter().find(|l| labels.iter().any(|x| x == *l)) else {
        let kinds: Vec<&str> = has.iter().copied().filter(|l| l.starts_with("kind:")).collect();
        return keep(if kinds.is_empty() { format!("#{n} has no kind label") } else { kinds.join(", ") });
    };
    let (next, _) = next_fallback(&list_of(ch.at("reviewer.fallback")), ch.s("reviewer.agent"), &[], |a| usable(a, ch.s("reviewer.access")));
    if next.is_empty() { return keep(format!("{hit}, but no usable reviewer.fallback agent")); }
    Route { agent: Some(next), why: hit.to_string() }
}
