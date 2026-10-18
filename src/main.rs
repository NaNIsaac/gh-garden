mod github;
mod local;
mod reconcile;
mod state;

use reconcile::{Finding, Status};
use std::path::{Path, PathBuf};
use std::process::{exit, Command};

const USAGE: &str = "\
gh garden — what differs between your GitHub account and your machines

USAGE
  gh garden reconcile [--root <dir>] [--plan] [--json]
  gh garden snapshot  [--root <dir>]

reconcile  Compare GitHub with this machine; also show what other machines
           last reported. Read-only. Exit 2 if work here isn't on GitHub.
  --plan   Print the commands that would fix each finding. Runs nothing.
  --json   Findings for this machine as JSON.
snapshot   reconcile, then record this machine's unsaved work in your
           secret gist \"gh-garden state\" so your other machines see it.

Root is --root, or `git config --global garden.root`.
Repos are expected at <root>/<owner>/<repo>.
Machine name is $GARDEN_MACHINE, `git config --global garden.machine`,
or the hostname.";

fn main() {
    // `gh garden reconcile | head` closes stdout early; that's not a crash.
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.to_string().contains("failed printing to stdout") {
            exit(0);
        }
        default(info)
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut root, mut json, mut plan, mut cmd) = (None, false, false, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().map(PathBuf::from),
            "--json" => json = true,
            "--plan" => plan = true,
            "-h" | "--help" | "help" => return println!("{USAGE}"),
            c if cmd.is_none() && !c.starts_with('-') => cmd = Some(c.to_string()),
            other => die(&format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }
    let snapshot = match cmd.as_deref() {
        Some("reconcile") | Some("status") | None => false,
        Some("snapshot") => true,
        Some(c) => die(&format!("unknown command: {c}\n\n{USAGE}")),
    };

    let root = root.or_else(config_root).unwrap_or_else(|| {
        die("no root. Pass --root <dir> or run: git config --global garden.root <dir>")
    });
    if !root.is_dir() {
        die(&format!("root is not a directory: {}", root.display()));
    }

    let findings = reconcile::run(&root).unwrap_or_else(|e| die(&format!("GitHub: {e}")));
    let unsafe_here = findings.iter().any(|f| f.status == Status::Unsaved);

    if json {
        println!("{}", serde_json::to_string_pretty(&findings).unwrap());
    } else if plan {
        print_plan(&findings);
    } else {
        print(&root, &findings);
        if snapshot {
            save(&root, findings);
        } else {
            print_elsewhere();
        }
    }
    if unsafe_here {
        exit(2);
    }
}

fn print(root: &Path, findings: &[Finding]) {
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    println!("root  {}  ({})\n", root.display(), state::machine());
    let mut last = None;
    for f in findings.iter().filter(|f| f.status != Status::Synced) {
        if last != Some(f.status) {
            let n = findings.iter().filter(|g| g.status == f.status).count();
            println!("{} ({n})", f.status.title());
            last = Some(f.status);
        }
        let repo = f.repo.as_deref().unwrap_or("-");
        let path = f.path.as_deref().map(rel).unwrap_or_default();
        println!("  {repo:<36} {path:<28} {}", f.detail);
    }
    let synced = findings.iter().filter(|f| f.status == Status::Synced).count();
    let unsaved = findings.iter().filter(|f| f.status == Status::Unsaved).count();
    let untracked = findings.iter().filter(|f| f.status == Status::Untracked).count();
    println!("\nSynced: {synced}");
    match (unsaved, untracked) {
        (0, 0) => println!("Safe to leave this machine: everything here is on GitHub."),
        (0, u) => println!("Safe to leave this machine, unless {u} repo(s) hold untracked files you need."),
        (n, _) => println!("NOT safe to leave this machine: {n} repo(s) have work that isn't on GitHub."),
    }
    let fixable = findings.iter().filter(|f| f.fix.is_some()).count();
    if fixable > 0 {
        println!("{fixable} finding(s) have a fix: gh garden reconcile --plan");
    }
}

fn print_plan(findings: &[Finding]) {
    for f in findings.iter().filter(|f| f.fix.is_some()) {
        let what = f.repo.as_deref().unwrap_or("");
        println!("# {}: {what} {}", f.status.title(), f.detail);
        println!("{}\n", f.fix.as_deref().unwrap());
    }
    let manual: Vec<_> = findings
        .iter()
        .filter(|f| f.fix.is_none() && !matches!(f.status, Status::Synced | Status::Untracked))
        .collect();
    if !manual.is_empty() {
        println!("# Needs you (no safe command):");
        for f in manual {
            let what = f.repo.clone().or_else(|| f.path.as_ref().map(|p| p.display().to_string().replace('\\', "/")));
            println!("#   {}: {} {}", f.status.title(), what.unwrap_or_default(), f.detail);
        }
    }
}

fn save(root: &Path, findings: Vec<Finding>) {
    let snap = state::Snapshot {
        machine: state::machine(),
        root: root.display().to_string(),
        at: state::now(),
        findings: findings.into_iter().filter(|f| state::keep(f.status)).collect(),
    };
    match state::save(&snap) {
        Ok(url) => println!("\nSnapshot of {} saved: {url}", snap.machine),
        Err(e) => die(&format!("snapshot not saved: {e}")),
    }
}

fn print_elsewhere() {
    let me = state::machine();
    let snaps = match state::load() {
        Ok(s) => s,
        Err(e) => return eprintln!("\n(other machines unknown: {e})"),
    };
    let others: Vec<_> = snaps.iter().filter(|s| !s.machine.eq_ignore_ascii_case(&me)).collect();
    if others.is_empty() {
        return println!("\nNo other machine has reported. Run `gh garden snapshot` on each one.");
    }
    for s in others {
        let unsaved: Vec<_> = s.findings.iter().filter(|f| f.status == Status::Unsaved).collect();
        println!("\nOn {} ({}, {}):", s.machine, s.root, state::ago(s.at));
        if unsaved.is_empty() {
            println!("  nothing unsaved");
        }
        for f in unsaved {
            println!("  {:<36} {}", f.repo.as_deref().unwrap_or("-"), f.detail);
        }
    }
}

fn config_root() -> Option<PathBuf> {
    let out = Command::new("git").args(["config", "--global", "garden.root"]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then(|| PathBuf::from(s))
}

fn die(msg: &str) -> ! {
    eprintln!("gh garden: {msg}");
    exit(1)
}
