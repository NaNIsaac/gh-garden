mod github;
mod local;
mod reconcile;

use reconcile::Status;
use std::path::PathBuf;
use std::process::{exit, Command};

const USAGE: &str = "\
gh garden — what differs between your GitHub account and this machine

USAGE
  gh garden reconcile [--root <dir>] [--json]

Root is <dir>, or `git config --global garden.root`.
Repos are expected at <root>/<owner>/<repo>. Nothing is fetched or changed.";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut root = None;
    let mut json = false;
    let mut cmd = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().map(PathBuf::from),
            "--json" => json = true,
            "-h" | "--help" | "help" => return println!("{USAGE}"),
            c if cmd.is_none() && !c.starts_with('-') => cmd = Some(c.to_string()),
            other => die(&format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }
    match cmd.as_deref() {
        Some("reconcile") | Some("status") | None => {}
        Some(c) => die(&format!("unknown command: {c}\n\n{USAGE}")),
    }

    let root = root.or_else(config_root).unwrap_or_else(|| {
        die("no root. Pass --root <dir> or run: git config --global garden.root <dir>")
    });
    if !root.is_dir() {
        die(&format!("root is not a directory: {}", root.display()));
    }

    let findings = reconcile::run(&root).unwrap_or_else(|e| die(&format!("GitHub: {e}")));
    if json {
        return println!("{}", serde_json::to_string_pretty(&findings).unwrap());
    }
    print(&root, &findings);
}

fn print(root: &std::path::Path, findings: &[reconcile::Finding]) {
    let rel = |p: &std::path::Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    println!("root  {}\n", root.display());
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
