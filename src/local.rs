//! Local side. Read-only: no fetch, no checkout, nothing that writes to a repo.

use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_DEPTH: usize = 4;
const SKIP: &[&str] = &["node_modules", "target", ".venv", "venv", "__pycache__"];

/// Every git working tree under `root`, not descending into repos.
pub fn find_repos(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(root, 0, &mut found);
    found.sort();
    found
}

fn walk(dir: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if dir.join(".git").exists() {
        found.push(dir.to_path_buf());
        return;
    }
    if depth >= MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if SKIP.contains(&name.as_ref()) || !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        walk(&e.path(), depth + 1, found);
    }
}

#[derive(Debug, Default, Clone)]
pub struct Local {
    pub path: PathBuf,
    /// `owner/repo` parsed from origin, if origin is on github.com.
    pub origin: Option<String>,
    pub origin_url: Option<String>,
    pub head: String,
    /// A `git worktree` checkout; branches and stashes belong to its main repo.
    pub worktree: bool,
    pub modified: usize,
    pub untracked: usize,
    /// Commits on local branches that no remote-tracking ref contains.
    pub unpushed: usize,
    pub stashes: usize,
}

pub fn inspect(path: &Path) -> Local {
    let origin_url = git(path, &["remote", "get-url", "origin"]);
    let mut l = Local {
        path: path.to_path_buf(),
        origin: origin_url.as_deref().and_then(parse_github),
        origin_url,
        worktree: std::fs::read_to_string(path.join(".git"))
            .is_ok_and(|s| s.replace('\\', "/").contains("/worktrees/")),
        ..Default::default()
    };
    if let Some(s) = git(path, &["status", "--porcelain=v2", "--branch"]) {
        for line in s.lines() {
            if let Some(h) = line.strip_prefix("# branch.head ") {
                l.head = h.to_string();
            } else if line.starts_with('?') {
                l.untracked += 1;
            } else if !line.starts_with('#') && !line.starts_with('!') {
                l.modified += 1;
            }
        }
    }
    l.unpushed = count(path, &["rev-list", "--count", "--branches", "--not", "--remotes"]);
    l.stashes = count(path, &["rev-list", "--walk-reflogs", "--count", "refs/stash"]);
    l
}

/// Ahead/behind of local `branch` against GitHub's `oid`, without fetching.
/// `None` means the oid is not in the local object store: GitHub has
/// commits this machine has never seen.
pub fn compare(path: &Path, branch: &str, oid: &str) -> Option<(usize, usize)> {
    git(path, &["cat-file", "-e", &format!("{oid}^{{commit}}")])?;
    let s = git(path, &["rev-list", "--left-right", "--count", &format!("refs/heads/{branch}...{oid}")])?;
    let mut it = s.split_whitespace().filter_map(|n| n.parse().ok());
    Some((it.next()?, it.next()?))
}

pub fn has_branch(path: &Path, branch: &str) -> bool {
    git(path, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).is_some()
}

fn count(path: &Path, args: &[&str]) -> usize {
    git(path, args).and_then(|s| s.trim().parse().ok()).unwrap_or(0)
}

fn git(path: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(path).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

/// `owner/repo` from any github.com remote URL form.
pub fn parse_github(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("git://github.com/"))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (o, r) = (parts.next()?, parts.next()?);
    (!o.is_empty() && !r.is_empty() && parts.next().is_none()).then(|| format!("{o}/{r}"))
}

#[cfg(test)]
mod tests {
    use super::parse_github;

    #[test]
    fn parses_remote_forms() {
        for url in [
            "https://github.com/NaNIsaac/QoraPhase.git",
            "https://github.com/NaNIsaac/QoraPhase",
            "git@github.com:NaNIsaac/QoraPhase.git",
            "ssh://git@github.com/NaNIsaac/QoraPhase",
        ] {
            assert_eq!(parse_github(url).as_deref(), Some("NaNIsaac/QoraPhase"), "{url}");
        }
        assert_eq!(parse_github("https://gitlab.com/a/b"), None);
    }
}
