//! Labels taken off the issue a task implements once its pull request merges (`pr.clearIssueLabels`, e.g.
//! `status:in-progress`): the issue is the one the task's description names (finish/closes.rs). Only labels the issue
//! has are removed, each said in runner.log ("issue 12: removed status:in-progress"); gh failing is said there too,
//! and never holds the task.
use crate::finish::shared::runner_log;
use crate::flow::git::gh;
use crate::json::*;
use crate::store::{repos::folder_of, settings::{lookup::work, values::list_of}};

pub fn clear(id: &str, t: &Obj) {
    let want: Vec<String> = list_of(work(Some(t)).at("pr.clearIssueLabels")).into_iter().filter(|l| l != "none" && !l.trim().is_empty()).collect();
    if want.is_empty() || !t.b("pr") { return; }
    let root = folder_of(id);
    let Some(n) = crate::finish::closes::issue_of(t, &root) else { return };
    let r = gh(&["issue", "view", &n.to_string(), "--json", "labels", "-q", ".labels[].name"], &root, 30000);
    if r.status != 0 { runner_log(id, &format!("issue {n}: labels not read: {}", r.stderr.lines().next().unwrap_or("").trim())); return; }
    let has: Vec<&str> = r.stdout.lines().map(str::trim).collect();
    for l in want.iter().filter(|l| has.contains(&l.as_str())) {
        let e = gh(&["issue", "edit", &n.to_string(), "--remove-label", l], &root, 30000);
        if e.status == 0 { runner_log(id, &format!("issue {n}: removed {l}")); }
        else { runner_log(id, &format!("issue {n}: could not remove {l}: {}", e.stderr.lines().next().unwrap_or("").trim())); }
    }
}
