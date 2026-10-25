//! Local side. Read-only: no fetch, no checkout, nothing that writes to a repo.

use std::collections::HashMap;
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
    /// Commits on local branches that no remote-tracking ref or known GitHub
    /// head contains.
    pub unpushed: usize,
    /// Local branches (with their commit count) that have no remote-tracking
    /// ref and whose GitHub head is unknown: never pushed, or never fetched.
    /// Their commits are not in `unpushed`; a fetch is needed to decide.
    pub unfetched: Vec<(String, usize)>,
    pub stashes: usize,
}

/// GitHub branch heads by `owner/repo` (lowercase): branch name -> commit oid.
pub type Heads = HashMap<String, HashMap<String, String>>;

pub fn inspect(path: &Path, heads: &Heads) -> Local {
    // Parsed from the redacted form: the credential is not part of the repository name, and the
    // raw value then never leaves this function.
    let origin_url = git(path, &["remote", "get-url", "origin"]).map(|u| redact_url(&u));
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
    let repo_heads = l.origin.as_ref().and_then(|o| heads.get(&o.to_lowercase()));
    (l.unpushed, l.unfetched) = count_unpushed(path, repo_heads);
    l.stashes = count(path, &["rev-list", "--walk-reflogs", "--count", "refs/stash"]);
    l
}

/// Commits on local branches that no remote-tracking ref contains. A branch with no tracking ref
/// is compared with GitHub's head of the same name when it is known, so commits already on the
/// remote are not counted. Branches that cannot be compared come back by name with their count.
fn count_unpushed(path: &Path, heads: Option<&HashMap<String, String>>) -> (usize, Vec<(String, usize)>) {
    let Some(branches) = git(path, &["for-each-ref", "--format=%(refname:short)", "refs/heads"]) else {
        return (0, Vec::new());
    };
    let (mut n, mut tracked, mut unfetched) = (0, Vec::new(), Vec::new());
    for b in branches.lines().filter(|b| !b.is_empty()) {
        let local = format!("refs/heads/{b}");
        if git(path, &["rev-parse", "--verify", "--quiet", &format!("refs/remotes/origin/{b}")]).is_some() {
            tracked.push(local);
            continue;
        }
        let pending = count(path, &["rev-list", "--count", &local, "--not", "--remotes"]);
        if pending == 0 {
            continue;
        }
        match heads.and_then(|h| h.get(b)).and_then(|oid| compare(path, b, oid)) {
            Some((ahead, _)) => n += ahead,
            None => unfetched.push((b.to_string(), pending)),
        }
    }
    if !tracked.is_empty() {
        let mut args = vec!["rev-list", "--count"];
        args.extend(tracked.iter().map(String::as_str));
        args.extend(["--not", "--remotes"]);
        n += count(path, &args);
    }
    (n, unfetched)
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
        .or_else(|| url.strip_prefix("ssh://github.com/"))
        .or_else(|| url.strip_prefix("git://github.com/"))?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (o, r) = (parts.next()?, parts.next()?);
    (!o.is_empty() && !r.is_empty() && parts.next().is_none()).then(|| format!("{o}/{r}"))
}

/// `user:password@` dropped from a URL before it is printed or saved. Covers the usual
/// `scheme://userinfo@host/...`; it does not cover a password holding an unencoded `/`, `?` or `#`,
/// a token in a query string, or the scp-like `user:password@host:path`.
fn redact_url(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let (authority, tail) = rest.find('/').map_or((rest, ""), |i| rest.split_at(i));
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    format!("{scheme}://{host}{tail}")
}

#[cfg(test)]
mod tests {
    use super::{parse_github, redact_url};

    #[test]
    fn redacts_credentials_only() {
        assert_eq!(redact_url("https://user:tok@gitlab.com/a/b.git"), "https://gitlab.com/a/b.git");
        assert_eq!(redact_url("https://tok@host"), "https://host");
        assert_eq!(redact_url("ssh://git@host/a/b"), "ssh://host/a/b");
        assert_eq!(redact_url("https://github.com/a/b"), "https://github.com/a/b");
        assert_eq!(redact_url("git@github.com:a/b.git"), "git@github.com:a/b.git");
        assert_eq!(redact_url("/srv/repos/b.git"), "/srv/repos/b.git");
    }

    /// A throw-away repository whose `origin` is `url`, so `inspect` reads it the way it reads a real one.
    fn repo_with_origin(tag: &str, url: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gh-garden-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for args in [vec!["init", "-q"], vec!["remote", "add", "origin", url]] {
            let status = std::process::Command::new("git").arg("-C").arg(&dir).args(&args).status().unwrap();
            assert!(status.success(), "git {args:?}");
        }
        dir
    }

    #[test]
    fn a_github_origin_with_credentials_is_still_github() {
        for (tag, url) in [
            ("https", "https://bob:s3cr3t-token@github.com/NaNIsaac/gh-garden.git"),
            ("tokenonly", "https://s3cr3t-token@github.com/NaNIsaac/gh-garden"),
            ("ssh", "ssh://git@github.com/NaNIsaac/gh-garden.git"),
        ] {
            let dir = repo_with_origin(tag, url);
            let local = super::inspect(&dir, &super::Heads::new());
            std::fs::remove_dir_all(&dir).unwrap();
            assert_eq!(local.origin.as_deref(), Some("NaNIsaac/gh-garden"), "{tag}");
            assert!(!local.origin_url.unwrap().contains("s3cr3t"), "{tag}");
        }
    }

    #[test]
    fn credentials_never_reach_the_stored_url() {
        let dir = repo_with_origin("gitlab", "https://alice:s3cr3t-pass@gitlab.example.com/o/r.git");
        let local = super::inspect(&dir, &super::Heads::new());
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(local.origin, None);
        assert_eq!(local.origin_url.as_deref(), Some("https://gitlab.example.com/o/r.git"));
    }

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

    fn git_ok(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git").arg("-C").arg(dir).args(args).status().unwrap();
        assert!(status.success(), "git {args:?}");
    }

    /// Two commits on `main`, no remote-tracking ref (never fetched), as in issue #5.
    fn unfetched_repo(tag: &str) -> (std::path::PathBuf, String) {
        let dir = repo_with_origin(tag, "https://github.com/NaNIsaac/gh-garden.git");
        git_ok(&dir, &["checkout", "-q", "-b", "main"]);
        for msg in ["one", "two"] {
            std::fs::write(dir.join("work"), msg).unwrap();
            git_ok(&dir, &["add", "work"]);
            git_ok(&dir, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", msg]);
        }
        let oid = super::git(&dir, &["rev-parse", "HEAD"]).unwrap();
        (dir, oid)
    }

    /// Commits already on the same-named GitHub branch are not unpushed work, even when the
    /// remote-tracking ref was never fetched.
    #[test]
    fn untracked_branch_on_remote_is_not_unpushed() {
        let (dir, oid) = unfetched_repo("on-remote");
        let heads = super::Heads::from([(
            "nanisaac/gh-garden".to_string(),
            std::collections::HashMap::from([("main".to_string(), oid)]),
        )]);
        let local = super::inspect(&dir, &heads);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(local.unpushed, 0);
        assert!(local.unfetched.is_empty());
    }

    /// A branch GitHub's head is not known for is reported by name with its count, not as pushed.
    #[test]
    fn unknown_remote_branch_is_reported() {
        let (dir, _) = unfetched_repo("unknown");
        let local = super::inspect(&dir, &super::Heads::new());
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(local.unpushed, 0);
        assert_eq!(local.unfetched, vec![("main".to_string(), 2)]);
    }
}
