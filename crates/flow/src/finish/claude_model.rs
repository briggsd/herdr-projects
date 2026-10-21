//! Which Claude model a task's issue labels choose (`worker.claudeModelByLabel`, `reviewer.claudeModelByLabel`): a
//! list of `label=model` pairs, the first whose label the issue has winning. Only for Claude: another program keeps its
//! own model. With no issue or none of those labels, the default; with gh failing too, and the reason is said.
use super::review_route::Issue;
use crate::json::*;
use crate::store::settings::values::list_of;

pub enum Pick { Default, Model { model: String, label: String }, Failed(String) }

pub fn pick(ch: &Obj, key: &str, issue: &Issue) -> Pick {
    let pairs: Vec<(String, String)> = list_of(ch.at(key)).into_iter().filter_map(|p| p.split_once('=').map(|(l, m)| (l.trim().to_string(), m.trim().to_string()))).filter(|(l, m)| !l.is_empty() && !m.is_empty()).collect();
    if pairs.is_empty() { return Pick::Default; }
    match issue {
        Ok((_, has)) => pairs.into_iter().find(|(l, _)| has.contains(l)).map(|(label, model)| Pick::Model { model, label }).unwrap_or(Pick::Default),
        Err(why) if why.is_empty() || why == "no issue" => Pick::Default,
        Err(why) => Pick::Failed(why.clone()),
    }
}
