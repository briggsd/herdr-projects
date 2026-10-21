//! Issue labels a task's pull request takes too (`pr.copyLabels`, e.g. `needs-human`): the issue is the one the task's
//! description names (finish/closes.rs). Copied when the board opens or finds the pull request, at each review's start
//! and as the task reaches the approval gate, so a label put on the issue mid-task still reaches the pull request before
//! the gate. Each label is added once (`prLabels` remembers it), and runner.log says so ("pr labels: added needs-human
//! from #12"). gh failing is said there too, and never holds the task.
use super::shared::runner_log;
use crate::json::*;
use crate::runner::Guard;
use crate::store::settings::lookup::work;
use crate::store::settings::values::list_of;
use std::path::Path;

pub fn copy(id: &str, g: &mut Guard, root: &Path) {
    let Ok(t) = g.check() else { return };
    let want: Vec<String> = list_of(work(Some(&t)).at("pr.copyLabels")).into_iter().filter(|l| l != "none").collect();
    let done: Vec<String> = t.arr("prLabels").iter().filter_map(|v| v.as_str().map(String::from)).collect();
    let pr = t.t("pr");
    if want.iter().all(|l| done.contains(l)) || pr.is_empty() { return; }
    let Some(n) = super::closes::issue_of(&t, root) else { return };
    let r = crate::flow::git::gh(&["issue", "view", &n.to_string(), "--json", "labels", "-q", ".labels[].name"], root, 30000);
    if r.status != 0 { runner_log(id, &format!("pr labels: gh failed on #{n}: {}", r.stderr.lines().next().unwrap_or("").trim())); return; }
    let has: Vec<&str> = r.stdout.lines().map(str::trim).collect();
    let mut added = vec![];
    for l in want.iter().filter(|l| has.contains(&l.as_str()) && !done.contains(l)) {
        let e = crate::flow::git::gh(&["pr", "edit", &pr, "--add-label", l], root, 30000);
        if e.status == 0 { runner_log(id, &format!("pr labels: added {l} from #{n}")); added.push(l.clone()); }
        else { runner_log(id, &format!("pr labels: could not add {l} from #{n}: {}", e.stderr.lines().next().unwrap_or("").trim())); }
    }
    if !added.is_empty() { let _ = g.set(|t| { let mut all = done.clone(); all.extend(added.clone()); t.put("prLabels", json!(all)); }); }
}
