//! A task left waiting for approval: after 30 min with no merge intent the Coordinator is told again (the task, its
//! pull request, its review's verdict line), then every 60 min it still waits; never for a task meant to merge.
use crate::common;
use crate::coordinator_messages::coord_world_with;
use common::*;
use serde_json::json;
use sw::json::*;
use sw::store::tasks::{update, Change};

fn queued(w: &World) -> Vec<String> { sw::coordinator::state::get(&w.socket).unwrap_or_default().arr("queue").iter().map(|q| q.t("said")).collect() }
fn log_of(id: &str) -> String { std::fs::read_to_string(sw::store::files::dir_of(id).join("runner.log")).unwrap_or_default() }
fn waiting(w: &World, mins: i64) -> String {
    let id = w.add("Fix the thing");
    let (so, at) = (w.socket.clone(), sw::clock::now() - mins * 60000);
    update(&id, |t| {
        t.put("stage", "approval"); t.put("pr", "https://github.com/o/r/pull/7"); t.put("link", json!({ "socket": so }));
        t.put("notice", json!({ "event": "ready to approve", "stage": "approval", "at": at }));
        t.put("reviews", json!([{ "round": 2, "pass": true, "text": "All four findings are fixed.\nMore." }]));
        Ok(Change::Write)
    }).unwrap();
    id
}
fn pass(w: &World) { let t: Vec<_> = sw::store::tasks::list(Default::default()); sw::approval_nudge::nudge_pass(&t, &w.socket); }

#[test]
fn after_30_min_the_coordinator_is_told_again_then_every_60_min() {
    let w = coord_world_with(json!({}));
    let id = waiting(&w, 31);
    pass(&w);
    let q = queued(&w);
    assert_eq!(q.len(), 1, "{q:?}");
    assert!(q[0].contains("has waited 31 min for approval: Fix the thing, https://github.com/o/r/pull/7. Review 2: All four findings are fixed. Approve it (board approve"), "{}", q[0]);
    assert!(log_of(&id).contains("approval: re-sent to coordinator (31 min waiting)"), "{}", log_of(&id));
    pass(&w);
    assert_eq!(queued(&w).len(), 1, "not again within the hour");
    update(&id, |t| { let a = t.at("approvalNudge").i("at") - 61 * 60000; t.put("approvalNudge", json!({ "at": a, "since": t.at("notice").i("at") })); Ok(Change::Write) }).unwrap();
    pass(&w);
    assert_eq!(queued(&w).len(), 2, "again after an hour");
}

#[test]
fn not_before_30_min_nor_for_a_task_meant_to_merge() {
    let w = coord_world_with(json!({}));
    let _young = waiting(&w, 20);
    let meant = waiting(&w, 45);
    update(&meant, |t| { t.put("mergeIntent", true); Ok(Change::Write) }).unwrap();
    pass(&w);
    assert!(queued(&w).is_empty(), "{:?}", queued(&w));
}
