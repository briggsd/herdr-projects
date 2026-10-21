//! What a pane's agent left running in the background: the processes under its programs and out of the terminal's
//! foreground (what `pane_reap` stops), and of those the jobs it started (`pane_jobs`), not its own helpers.
use super::api::Herdr;
use super::cli::kids;
use crate::json::*;

impl Herdr {
    /// What the pane's agent started that still runs in the background: the same processes `pane_reap` would stop, each
    /// job once (the top of its tree), but not the program's own helpers it keeps between turns: one wired to the
    /// program on its input and output (an MCP server, procs.rs), an MCP or language server by its name, or the program
    /// itself. Unknown (herdr does not answer): none.
    pub fn pane_jobs(&self, id: &str, program: &str) -> Vec<Job> {
        let Some(i) = self.process_info(id) else { return vec![] };
        let (front, tops, _) = strays(&i);
        let fake = super::cli::fake();
        tops.into_iter().filter_map(|pid| {
            let command = command_of(pid);
            let first = command.split_whitespace().next().unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
            if super::procs::wired_to(pid, &front) == Some(true) || helper_named(&command) || (!program.is_empty() && first == program) { return None; }
            let age = match &fake { Some(f) => f.age(pid), None => std::process::Command::new("ps").args(["-o", "etimes=", "-p", &pid.to_string()]).output().ok().and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok()) };
            Some(Job { pid, command, age })
        }).collect()
    }
}

/// A job the agent left running in the background (`pane_jobs`): its pid, its command (200 characters at most) and
/// how long it has run, in seconds, where the system tells.
#[derive(Clone, Debug)]
pub struct Job { pub pid: i64, pub command: String, pub age: Option<i64> }
/// Under the pane's foreground programs, what is out of the foreground (not the shell): (the foreground, the tops of
/// those trees, every process in them).
pub(super) fn strays(i: &Value) -> (Vec<i64>, Vec<i64>, Vec<i64>) {
    let front: Vec<i64> = i.arr("foreground_processes").iter().filter(|p| crate::gate::is_int(p.at("pid"))).map(|p| p.i("pid")).collect();
    let shell = i.i("shell_pid");
    let (mut tops, mut strays) = (vec![], vec![]);
    let mut todo: Vec<(i64, bool)> = front.iter().copied().filter(|p| *p != shell).map(|p| (p, true)).collect();
    while let Some((pid, fg)) = todo.pop() {
        for k in kids(pid).unwrap_or_default() { if !front.contains(&k) && k != shell && !strays.contains(&k) { strays.push(k); if fg { tops.push(k); } todo.push((k, false)); } }
    }
    (front, tops, strays)
}
/// The command a process runs, 200 characters at most: for a shell an agent's tool wrapped it in (Claude Code's
/// `bash -c source <snapshot> … && eval '<command>' < /dev/null && pwd -P …`), the command it evaluates.
pub(super) fn command_of(pid: i64) -> String {
    if super::cli::fake().is_some() { return format!("process {pid}"); }
    let o = std::process::Command::new("ps").args(["-ww", "-o", "args=", "-p", &pid.to_string()]).output().ok();
    o.map(|o| unwrapped(String::from_utf8_lossy(&o.stdout).trim()).chars().take(200).collect()).unwrap_or_default()
}
fn unwrapped(args: &str) -> String {
    let Some((_, rest)) = args.split_once(" && eval '") else { return args.to_string() };
    let body = rest.rfind("' < /dev/null").or_else(|| rest.rfind('\'')).map(|e| &rest[..e]).unwrap_or(rest);
    body.replace("'\\''", "'")
}
/// An agent program's own long-lived helper, by its command: an MCP server or a language server.
fn helper_named(command: &str) -> bool {
    let c = command.to_lowercase();
    ["mcp", "language-server", "languageserver", "langserver", "rust-analyzer", "gopls", "clangd", "pyright", "jdtls", "lsp-server", "sourcekit-lsp"].iter().any(|h| c.contains(h))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_tool_s_shell_says_the_command_it_evaluates() {
        let raw = "/usr/bin/bash -c source /home/u/.claude/shell-snapshots/snapshot-bash-1.sh 2>/dev/null || true && shopt -u extglob 2>/dev/null || true && eval 'bash tools/check.sh > /tmp/c.txt 2>&1; echo '\\''done'\\''' < /dev/null && pwd -P >| /tmp/claude-1-cwd";
        assert_eq!(super::unwrapped(raw), "bash tools/check.sh > /tmp/c.txt 2>&1; echo 'done'");
        assert_eq!(super::unwrapped("cargo test -q"), "cargo test -q");
        assert!(super::helper_named("node /x/node_modules/@playwright/mcp/cli.js") && super::helper_named("rust-analyzer") && !super::helper_named("bash tools/check.sh"));
    }
}
