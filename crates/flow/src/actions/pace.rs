//! The Claude spend pacer (`board.claudePace`, e.g. `16/h over 3h`): before a queued task starts, Claude Code's spend
//! over the window (sum(increase(claude_code_cost_usage_USD_total[3h])) / 3, every host that reports to the
//! Prometheus at `board.claudePaceFrom`) is read. At or over the budget nothing new starts, and the next poll looks
//! again; running tasks, reviews and fix rounds go on. Prometheus not answering starts the task. A reading is kept a
//! minute (pace.json in the data dir), so the board's polls do not ask each time. `board.claudePaceSkip` leaves some
//! projects' sessions out: those `claude_session_project{project=...}` names (agent-obs's map from each session's
//! transcript directory; hearth is not freelancer's, owner 2026-10-24).
use crate::json::*;
use crate::store::files::data_dir;
use crate::store::settings::{lookup::{setting, setting_text}, values::list_of};

pub enum Pace { Off, Start(String), Hold(String) }

/// `16/h over 3h`: the budget in dollars an hour and the window in hours (3 when not said).
fn budget() -> Option<(f64, i64)> {
    let s = setting_text("board.claudePace", None);
    let nums: Vec<f64> = regex::Regex::new(r"\d+(?:\.\d+)?").unwrap().find_iter(&s).filter_map(|m| m.as_str().parse().ok()).collect();
    let b = *nums.first().filter(|b| **b > 0.0)?;
    Some((b, nums.get(1).map(|h| h.round() as i64).filter(|h| *h > 0).unwrap_or(3)))
}

/// The PromQL for dollars an hour over the last `hours`, without the sessions of the `skip` projects.
pub fn query(hours: i64, skip: &[String]) -> String {
    let cost = format!("increase(claude_code_cost_usage_USD_total[{hours}h])");
    let skip: Vec<String> = skip.iter().filter(|p| !p.is_empty() && *p != "none").map(|p| regex::escape(p).replace('\\', "\\\\")).collect();
    if skip.is_empty() { return format!("sum({cost}) / {hours}"); }
    format!("sum(sum by (session_id) ({cost}) unless on(session_id) claude_session_project{{project=~\"{}\"}}) / {hours}", skip.join("|"))
}

/// Dollars an hour over the last `hours`, or why it could not be read.
fn spend(hours: i64) -> Result<f64, String> {
    let cache = data_dir().join("pace.json");
    let now = crate::clock::now();
    let q = query(hours, &list_of(&setting("board.claudePaceSkip", None)));
    if let Some(c) = std::fs::read_to_string(&cache).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok()) {
        if c.i("hours") == hours && (!c.b("query") || c.s("query") == q) && now - c.i("at") < 60000 { return Ok(c.f("rate")); }
    }
    let from = setting_text("board.claudePaceFrom", None);
    let from = if from.is_empty() || from == "none" { "http://localhost:9090".to_string() } else { from.trim_end_matches('/').to_string() };
    let out = std::process::Command::new("curl").args(["-fsS", "--max-time", "5", "-G", &format!("{from}/api/v1/query"), "--data-urlencode", &format!("query={q}")]).output().map_err(|e| e.to_string())?;
    if !out.status.success() { return Err(String::from_utf8_lossy(&out.stderr).trim().to_string()); }
    let v: Value = serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
    if v.s("status") != "success" { return Err(format!("Prometheus said {}", v.s("status"))); }
    let rate = v.at("data").arr("result").first().and_then(|r| r.at("value").as_array().and_then(|a| a.get(1)).and_then(|x| x.as_str()).and_then(|x| x.parse::<f64>().ok())).unwrap_or(0.0);
    let _ = std::fs::write(&cache, json!({ "at": now, "hours": hours, "rate": rate, "query": q }).to_string());
    Ok(rate)
}

pub fn check() -> Pace {
    let Some((b, h)) = budget() else { return Pace::Off };
    match spend(h) {
        Ok(x) if x >= b => Pace::Hold(format!("pace: hold (spend ${x:.2}/h >= ${b}/h over {h}h)")),
        Ok(x) => Pace::Start(format!("pace: start (spend ${x:.2}/h < ${b}/h)")),
        Err(e) => Pace::Start(format!("pace: start (spend not read: {})", e.lines().next().unwrap_or(""))),
    }
}
