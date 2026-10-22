//! Codex's weekly allowance, paced (`reviewer.codexPace`): a task whose issue has one of those labels is reviewed by
//! `reviewer.agent` (Codex) while the allowance used is under the share of the week gone by (plus 5 points), and by the
//! first usable agent on `reviewer.fallback` otherwise. The reading is Codex's own: the newest `rate_limits` its
//! session files under `$CODEX_HOME/sessions` (~/.codex) say, the weekly window's used percent and reset time. A
//! reading over an hour old (Codex has not run since) sends one review to Codex anyway, so the reading comes back; the
//! next such review within the hour is paced on the old reading. Said in runner.log as the review's why.
use super::hand_over::usable;
use super::review_route::Route;
use crate::json::*;
use crate::store::handovers::next_fallback;
use crate::store::settings::values::list_of;
use std::path::{Path, PathBuf};

const MARGIN: f64 = 5.0;
const STALE_S: i64 = 3600;

/// Codex's weekly allowance at one moment: the percent used, the percent of the window gone by, when it was read.
pub struct Reading { pub used: f64, pub elapsed: f64, pub at_s: i64 }

/// The paced route, or None when the pace does not apply (no labels set, Codex not the reviewer, no such label).
pub fn pace(ch: &Obj, has: &[String]) -> Option<Route> {
    let labels: Vec<String> = list_of(ch.at("reviewer.codexPace")).into_iter().filter(|l| l != "none").collect();
    if labels.is_empty() || crate::store::settings::values::agent_of(ch.s("reviewer.agent")).kind != "codex" { return None; }
    if !has.iter().any(|l| labels.contains(l)) { return None; }
    let now_s = crate::clock::now() / 1000;
    let codex = |why: String| Some(Route { agent: None, why });
    let r = reading(&codex_home(), now_s);
    let probe = crate::store::files::data_dir().join("codex-pace-probe");
    let probed = std::fs::read_to_string(&probe).ok().and_then(|s| s.trim().parse::<i64>().ok()).is_some_and(|at| now_s - at < STALE_S);
    let stale = r.as_ref().is_none_or(|r| now_s - r.at_s > STALE_S);
    if stale && !probed {
        let _ = std::fs::write(&probe, now_s.to_string());
        return codex(match &r { Some(r) => format!("pace: reading {}m old, one review to refresh it", (now_s - r.at_s) / 60), None => "pace: no reading yet, one review to get one".into() });
    }
    let fallback = || next_fallback(&list_of(ch.at("reviewer.fallback")), ch.s("reviewer.agent"), &[], |a| usable(a, ch.s("reviewer.access"))).0;
    let Some(r) = r else {
        let a = fallback();
        return Some(Route { agent: (!a.is_empty()).then_some(a), why: "pace: no reading yet".into() });
    };
    if r.used < r.elapsed + MARGIN { return codex(format!("pace: used {:.0}% < elapsed {:.0}% + {MARGIN:.0}", r.used, r.elapsed)); }
    let a = fallback();
    if a.is_empty() { return codex(format!("pace: used {:.0}% >= elapsed {:.0}% + {MARGIN:.0}, but no usable reviewer.fallback agent", r.used, r.elapsed)); }
    Some(Route { agent: Some(a), why: format!("pace: used {:.0}% >= elapsed {:.0}% + {MARGIN:.0}", r.used, r.elapsed) })
}

fn codex_home() -> PathBuf { std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex")) }

/// The newest weekly reading: the last one in the newest session file that has one (the last two days' folders, newest
/// file first, its last 256 KB only: Codex says its rate limits at every turn, and a session file runs to tens of MB).
pub fn reading(home: &Path, now_s: i64) -> Option<Reading> {
    let mut days: Vec<PathBuf> = vec![];
    for y in sorted_dirs(&home.join("sessions")).into_iter().take(2) { for m in sorted_dirs(&y).into_iter().take(2) { days.extend(sorted_dirs(&m).into_iter().take(2)); } }
    days.sort(); days.reverse();
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = days.iter().take(2).flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten()).filter(|e| e.path().extension().is_some_and(|x| x == "jsonl")).filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
    files.sort(); files.reverse();
    files.into_iter().take(6).find_map(|(_, f)| tail(&f).lines().rev().filter(|l| l.contains("\"rate_limits\"")).find_map(|l| weekly(l, now_s)))
}
fn tail(f: &Path) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut h) = std::fs::File::open(f) else { return String::new() };
    let len = h.metadata().map(|m| m.len()).unwrap_or(0);
    let from = len.saturating_sub(256 * 1024);
    let mut buf = vec![];
    if h.seek(SeekFrom::Start(from)).is_err() || h.read_to_end(&mut buf).is_err() { return String::new(); }
    let text = String::from_utf8_lossy(&buf).into_owned();
    if from > 0 { text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or_default() } else { text }
}
fn sorted_dirs(d: &Path) -> Vec<PathBuf> { let mut v: Vec<PathBuf> = std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect(); v.sort(); v.reverse(); v }
fn weekly(line: &str, now_s: i64) -> Option<Reading> {
    let v: Value = serde_json::from_str(line).ok()?;
    let rl = v.at("payload").at("rate_limits");
    let w = [rl.at("primary"), rl.at("secondary")].into_iter().find(|w| w.i("window_minutes") == 10080)?;
    let at_s = chrono::DateTime::parse_from_rfc3339(v.s("timestamp")).ok()?.timestamp();
    let (len, reset) = (w.i("window_minutes") * 60, w.i("resets_at"));
    let elapsed = if reset > 0 { ((now_s - (reset - len)) as f64 / len as f64 * 100.0).clamp(0.0, 100.0) } else { 0.0 };
    Some(Reading { used: w.at("used_percent").as_f64()?, elapsed, at_s })
}
