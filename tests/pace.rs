//! The Claude spend pacer (board.claudePace): under the budget a queued task starts and says so; at or over it the
//! task waits, said once, and starts at a later poll once the spend is under; Prometheus not answering starts it.
use crate::common;
use common::*;
use serde_json::json;
use sw::json::*;

fn log_of(id: &str) -> String { std::fs::read_to_string(sw::store::files::dir_of(id).join("runner.log")).unwrap_or_default() }
/// Prometheus's reading, as the pacer keeps it for a minute.
fn reads(rate: f64) { std::fs::write(sw::store::files::data_dir().join("pace.json"), json!({ "at": sw::clock::now(), "hours": 3, "rate": rate }).to_string()).unwrap(); }
fn paced(rate: Option<f64>) -> (World, String) {
    // Nothing answers at that port: a reading not kept is a failed one.
    let w = world_with(Opts { repo: true, settings: json!({ "review.when": "never", "board.claudePace": "16/h over 3h", "board.claudePaceFrom": "http://127.0.0.1:9" }), ..Default::default() });
    if let Some(r) = rate { reads(r); }
    let id = w.add("Fix the thing");
    delegate(&id);
    (w, id)
}

#[test]
fn under_the_budget_a_task_starts_and_says_so() {
    let (_w, id) = paced(Some(9.5));
    assert_ne!(settle(&id).s("stage"), "queued");
    assert!(log_of(&id).contains("pace: start (spend $9.50/h < $16/h)"), "{}", log_of(&id));
}

#[test]
fn at_or_over_it_the_task_waits_said_once_and_starts_once_the_spend_is_under() {
    let (_w, id) = paced(Some(16.0));
    let socket = task(&id).at("link").t("socket");
    for _ in 0..3 { sw::actions::start::start_queued(&socket); }
    let t = task(&id);
    assert_eq!(t.s("stage"), "queued", "{}", t.s("msg"));
    assert_eq!(log_of(&id).matches("pace: hold (spend $16.00/h >= $16/h over 3h)").count(), 1, "{}", log_of(&id));
    assert!(t.s("msg").starts_with("Waiting: Claude spend $16.00/h >= $16/h"), "{}", t.s("msg"));
    reads(3.0);
    sw::actions::start::start_queued(&socket);
    let t = settle(&id);
    assert_ne!(t.s("stage"), "queued");
    assert!(log_of(&id).contains("pace: start (spend $3.00/h < $16/h)") && !t.b("paceHeld"), "{}", log_of(&id));
}

#[test]
fn prometheus_not_answering_starts_it_and_says_why() {
    let (_w, id) = paced(None);
    assert_ne!(settle(&id).s("stage"), "queued");
    assert!(log_of(&id).contains("pace: start (spend not read: curl: (7)"), "{}", log_of(&id));
}
