//! The pull request's plain "Closes #n" line, for a task made from a GitHub issue. The issue is the one the task's
//! description names in the line the Coordinator writes for it: "Implements GitHub issue owner/repo#n
//! (https://github.com/owner/repo/issues/n)". Nothing else in the text counts: a number in a sentence, a pull request,
//! "the next stage of" an issue.
use crate::json::*;
use crate::store::repos::git_out;
use std::path::Path;

/// The issue the task implements, when its description names one of the repo's own issues that way.
pub fn issue_of(t: &Obj, root: &Path) -> Option<u64> {
    let re = regex::Regex::new(r"^Implements GitHub issue ([\w.-]+/[\w.-]+)#(\d+) \(https://github\.com/([\w.-]+/[\w.-]+)/issues/(\d+)\)").unwrap();
    let c = t.s("description").lines().find_map(|l| re.captures(l.trim_end()))?;
    if c[1] != c[3] || c[2] != c[4] { return None; }
    // On another GitHub repo, "Closes #n" would close this repo's issue of that number.
    if let Some(own) = git_out(root, &["remote", "get-url", "origin"]).ok().as_deref().and_then(github_repo) {
        if !own.eq_ignore_ascii_case(&c[1]) { return None; }
    }
    c[2].parse().ok().filter(|n| *n > 0)
}

/// "owner/repo" of a GitHub remote's URL (https or ssh), or None for any other remote.
fn github_repo(url: &str) -> Option<String> {
    let rest = url.trim().split_once("github.com").map(|(_, r)| r)?.trim_start_matches([':', '/']);
    let rest = rest.strip_suffix(".git").unwrap_or(rest).trim_end_matches('/');
    rest.contains('/').then(|| rest.to_string())
}

/// The body with "Closes #n" as a line of its own, unless a line already is exactly that (trailing spaces allowed):
/// after the leading summary bullets when the body starts with some, else at the top. None when it was there.
pub fn with_closes(body: &str, n: u64) -> Option<String> {
    let mut out = placed(body, n)?;
    if body.ends_with('\n') && !out.ends_with('\n') { out.push('\n'); }
    Some(out)
}
fn placed(body: &str, n: u64) -> Option<String> {
    let line = format!("Closes #{n}");
    if body.lines().any(|l| l.trim_end() == line) { return None; }
    let lines: Vec<&str> = body.lines().collect();
    let bullet = |l: &str| { let l = l.trim_start(); l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") };
    if lines.first().is_some_and(|l| bullet(l)) {
        // The list runs to its first blank line, or to a line that is neither a bullet nor indented under one.
        let end = lines.iter().position(|l| l.trim().is_empty() || !(bullet(l) || l.starts_with([' ', '\t']))).unwrap_or(lines.len());
        let (head, tail) = lines.split_at(end);
        let tail = tail.join("\n");
        let tail = tail.trim_start_matches('\n');
        return Some(if tail.is_empty() { format!("{}\n\n{line}", head.join("\n")) } else { format!("{}\n\n{line}\n\n{tail}", head.join("\n")) });
    }
    Some(if body.trim().is_empty() { line } else { format!("{line}\n\n{body}") })
}

