//! GitHub account vs this machine. Produces findings; never changes anything.

use crate::{github, local};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Dirty files, unpushed commits or stashes: leaving this machine loses them.
    Unsaved,
    /// Only untracked files: often build junk, sometimes a forgotten new file.
    Untracked,
    Diverged,
    Behind,
    NotCloned,
    Renamed,
    Misplaced,
    LocalOnly,
    NotGit,
    Synced,
}

impl Status {
    pub fn title(self) -> &'static str {
        match self {
            Status::Unsaved => "Work not on GitHub",
            Status::Untracked => "Only untracked files",
            Status::Diverged => "Diverged from GitHub",
            Status::Behind => "Behind GitHub",
            Status::NotCloned => "On GitHub, not cloned here",
            Status::Renamed => "Renamed on GitHub, origin is stale",
            Status::Misplaced => "Not at <root>/<owner>/<repo>",
            Status::LocalOnly => "Local only",
            Status::NotGit => "Not a git repo (under an owner folder)",
            Status::Synced => "Synced",
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Finding {
    pub status: Status,
    pub repo: Option<String>,
    pub path: Option<PathBuf>,
    pub detail: String,
    /// A shell command that would resolve this. Printed by `--plan`, never run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

impl Finding {
    fn fix(mut self, cmd: String) -> Self {
        self.fix = Some(cmd);
        self
    }
}

pub fn run(root: &Path) -> Result<Vec<Finding>, String> {
    let remotes = github::list_repos()?;
    let by_id: HashMap<String, &github::Repo> =
        remotes.iter().map(|r| (r.name_with_owner.to_lowercase(), r)).collect();

    let paths = local::find_repos(root);
    let locals: Vec<local::Local> = std::thread::scope(|s| {
        let handles: Vec<_> = paths.iter().map(|p| s.spawn(|| local::inspect(p))).collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let find = |status, repo: Option<&str>, path: Option<&Path>, detail: String| Finding {
        status,
        repo: repo.map(str::to_string),
        path: path.map(Path::to_path_buf),
        detail,
        fix: None,
    };

    // Origins the account listing doesn't know: maybe renamed, maybe someone else's.
    let resolved: HashMap<String, Option<String>> = std::thread::scope(|s| {
        let unknown: HashSet<&String> = locals
            .iter()
            .filter_map(|l| l.origin.as_ref())
            .filter(|id| !by_id.contains_key(&id.to_lowercase()))
            .collect();
        let handles: Vec<_> = unknown
            .into_iter()
            .map(|id| (id.clone(), s.spawn(move || github::canonical_name(id))))
            .collect();
        handles.into_iter().map(|(id, h)| (id, h.join().unwrap())).collect()
    });

    let compares: Vec<Option<(String, Option<(usize, usize)>)>> = std::thread::scope(|s| {
        let handles: Vec<_> = locals
            .iter()
            .map(|l| {
                let id = l.origin.as_ref().map(|o| {
                    resolved.get(o).cloned().flatten().unwrap_or_else(|| o.clone())
                });
                let remote = id.and_then(|i| by_id.get(&i.to_lowercase()).copied());
                s.spawn(move || {
                    let b = remote?.default_branch_ref.as_ref()?;
                    local::has_branch(&l.path, &b.name)
                        .then(|| (b.name.clone(), local::compare(&l.path, &b.name, &b.target.oid)))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });

    for (l, cmp) in locals.iter().zip(compares) {
        let p = Some(l.path.as_path());
        let Some(origin) = &l.origin else {
            let detail = match &l.origin_url {
                Some(u) => format!("origin is not GitHub: {u}"),
                None => "no origin remote".into(),
            };
            out.push(find(Status::LocalOnly, None, p, detail));
            continue;
        };
        let id = match by_id.get(&origin.to_lowercase()) {
            Some(r) => r.name_with_owner.clone(),
            None => match resolved.get(origin).cloned().flatten() {
                Some(new) if by_id.contains_key(&new.to_lowercase()) => {
                    out.push(
                        find(Status::Renamed, Some(&new), p, format!("origin still says {origin}"))
                            .fix(format!("git -C {} remote set-url origin https://github.com/{new}.git", q(&l.path))),
                    );
                    by_id[&new.to_lowercase()].name_with_owner.clone()
                }
                Some(other) => {
                    out.push(find(Status::LocalOnly, Some(&other), p, "not in your account".into()));
                    continue;
                }
                None => {
                    out.push(find(Status::LocalOnly, Some(origin), p, "origin repo not found on GitHub".into()));
                    continue;
                }
            },
        };
        seen.insert(id.to_lowercase());
        let r = by_id[&id.to_lowercase()];
        let before = out.len();

        // A worktree shares branches, stashes and history with its main
        // checkout, which reports those; only its own files are news.
        let expected = Path::new(r.owner()).join(r.name());
        if !l.worktree && !same_path(&l.path, &root.join(&expected)) {
            let target = root.join(&expected);
            let f = find(Status::Misplaced, Some(&id), p, format!("expected {}", expected.display()));
            out.push(if target.exists() { f } else { f.fix(format!("mv {} {}", q(&l.path), q(&target))) });
        }

        let mut unsaved = Vec::new();
        let mut note = |n: usize, what: &str| {
            if n > 0 {
                unsaved.push(format!("{n} {what}"));
            }
        };
        note(l.modified, "modified");
        if !l.worktree {
            note(l.unpushed, "unpushed commits");
            note(l.stashes, "stashes");
        }
        let on = if l.worktree { format!("worktree on {}", l.head) } else { format!("on {}", l.head) };
        if !unsaved.is_empty() {
            if l.untracked > 0 {
                unsaved.push(format!("{} untracked", l.untracked));
            }
            let f = find(Status::Unsaved, Some(&id), p, format!("{} ({on})", unsaved.join(", ")));
            // Only clean, committed work has an obvious fix; edits need a human.
            // A push GitHub would reject (diverged, or commits never fetched) isn't a fix.
            let behind = matches!(&cmp, Some((_, None))) || matches!(&cmp, Some((_, Some((_, b)))) if *b > 0);
            let pushable =
                l.modified == 0 && l.stashes == 0 && l.unpushed > 0 && !behind && !l.head.starts_with('(');
            out.push(if pushable { f.fix(format!("git -C {} push origin {}", q(&l.path), l.head)) } else { f });
        } else if l.untracked > 0 {
            out.push(find(Status::Untracked, Some(&id), p, format!("{} untracked ({on})", l.untracked)));
        }

        // Fast-forward only when it can't touch work: on that branch, clean tree.
        let pull = |branch: &str| {
            (l.head == branch && l.modified == 0)
                .then(|| format!("git -C {} pull --ff-only", q(&l.path)))
                .unwrap_or_else(|| format!("git -C {} fetch origin", q(&l.path)))
        };
        match cmp.filter(|_| !l.worktree) {
            Some((branch, None)) => out.push(
                find(Status::Behind, Some(&id), p, format!("GitHub {branch} has commits never fetched here"))
                    .fix(pull(&branch)),
            ),
            Some((branch, Some((ahead, behind)))) if behind > 0 => {
                out.push(if ahead > 0 {
                    find(Status::Diverged, Some(&id), p, format!("{branch}: {ahead} ahead, {behind} behind"))
                } else {
                    find(Status::Behind, Some(&id), p, format!("{branch}: {behind} behind")).fix(pull(&branch))
                });
            }
            _ => {}
        }

        if out.len() == before {
            out.push(find(Status::Synced, Some(&id), p, String::new()));
        }
    }

    for r in &remotes {
        if !seen.contains(&r.name_with_owner.to_lowercase()) && !r.is_archived {
            let detail = if r.is_fork { "fork".into() } else { String::new() };
            let target = root.join(r.owner()).join(r.name());
            out.push(
                find(Status::NotCloned, Some(&r.name_with_owner), None, detail)
                    .fix(format!("gh repo clone {} {}", r.name_with_owner, q(&target))),
            );
        }
    }

    // Leftovers sitting where repos should be: <root>/<owner>/<dir> with no .git.
    let owners: HashSet<&str> = remotes.iter().map(|r| r.owner()).collect();
    for owner in owners {
        let Ok(entries) = std::fs::read_dir(root.join(owner)) else { continue };
        for e in entries.flatten() {
            let dir = e.path();
            if dir.is_dir() && !dir.join(".git").exists() && !paths.iter().any(|p| p.starts_with(&dir)) {
                out.push(find(Status::NotGit, None, Some(&dir), String::new()));
            }
        }
    }

    out.sort_by(|a, b| (a.status, &a.repo, &a.path).cmp(&(b.status, &b.repo, &b.path)));
    Ok(out)
}

/// Quoted, forward-slashed path: pastes into bash and PowerShell alike.
fn q(p: &Path) -> String {
    format!("\"{}\"", p.to_string_lossy().replace('\\', "/"))
}

/// Windows and macOS filesystems are case-insensitive; GitHub names are too.
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase();
    norm(a) == norm(b)
}
