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
  gh garden [reconcile] [--root <dir>] [--plan] [--json]
  gh garden snapshot    [--root <dir>]
  gh garden path [<query>]
  gh garden open <query>

reconcile  Compare GitHub with this machine, then show what your other
           machines last reported. Never touches a repo. Exit 2 if work
           here isn't on GitHub. With the configured root it also leaves
           this machine's note in your secret gist \"gh-garden state\"
           (stop that with `git config --global garden.sync false`).
  --plan   Print the commands that would fix each finding. Runs nothing.
  --json   Findings for this machine as JSON.
snapshot   reconcile and leave the note, whatever the root.
path       Path of the local repo matching <query> (all repos if none):
             cd (gh garden path qoraph)
open       Open that repo in `git config garden.editor` (default: code).

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
    let (mut root, mut json, mut plan) = (None, false, false);
    let mut pos: Vec<&str> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--root" => root = it.next().map(PathBuf::from),
            "--json" => json = true,
            "--plan" => plan = true,
            "-h" | "--help" | "help" => return println!("{USAGE}"),
            c if !c.starts_with('-') => pos.push(c),
            other => die(&format!("unknown argument: {other}\n\n{USAGE}")),
        }
    }
    let configured = config_root();
    let root = root.map(native).or_else(|| configured.clone()).unwrap_or_else(|| {
        die("no root. Pass --root <dir> or run: git config --global garden.root <dir>")
    });
    if !root.is_dir() {
        die(&format!("root is not a directory: {}", root.display()));
    }

    let snapshot = match pos.as_slice() {
        [] | ["reconcile" | "status"] => false,
        ["snapshot"] => true,
        ["path", q @ ..] if q.len() <= 1 => return navigate(&root, q.first().copied(), false),
        ["open", q] => return navigate(&root, Some(q), true),
        _ => die(&format!("unknown command: {}\n\n{USAGE}", pos.join(" "))),
    };
    // Every look from the usual root refreshes this machine's note, so the
    // others never see a stale one just because nobody ran `snapshot`.
    // `git config --global garden.sync false` stops this implicit upload; `snapshot` still writes.
    let usual_root = configured.is_some_and(|c| reconcile::same_path(&c, &root));
    let save = snapshot || (usual_root && git_config("garden.sync").as_deref() != Some("false"));

    let findings = reconcile::run(&root).unwrap_or_else(|e| die(&format!("GitHub: {e}")));
    let unsafe_here = findings.iter().any(|f| f.status == Status::Unsaved);

    if json {
        println!("{}", serde_json::to_string_pretty(&findings).unwrap());
    } else if plan {
        print_plan(&findings);
    } else {
        print(&root, &findings);
        sync(&root, &findings, save);
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

fn sync(root: &Path, findings: &[Finding], save: bool) {
    let me = state::machine();
    let snap = save.then(|| state::Snapshot {
        machine: me.clone(),
        root: root.display().to_string(),
        at: state::now(),
        findings: findings.iter().filter(|f| state::keep(f.status)).cloned().collect(),
    });
    let snaps = match state::sync(snap.as_ref()) {
        Ok(s) => s,
        Err(e) => return eprintln!("\n(other machines unknown, nothing saved: {e})"),
    };
    if save {
        println!("Note left for your other machines as \"{me}\".");
    }
    let others: Vec<_> = snaps.iter().filter(|s| !s.machine.eq_ignore_ascii_case(&me)).collect();
    if others.is_empty() {
        return println!("\nNo other machine has reported. Run `gh garden` on each one.");
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

/// Local only, no GitHub call: fast enough for `cd (gh garden path x)`.
fn navigate(root: &Path, query: Option<&str>, open: bool) {
    let repos = local::find_repos(root);
    let rel = |p: &Path| p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let Some(q) = query.map(str::to_lowercase) else {
        return repos.iter().for_each(|p| println!("{}", p.display()));
    };
    // Exact folder name wins; otherwise anything whose <owner>/<repo> contains it.
    let exact: Vec<_> = repos
        .iter()
        .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == q))
        .collect();
    let hits = if exact.is_empty() { repos.iter().filter(|p| rel(p).to_lowercase().contains(&q)).collect() } else { exact };
    let path = match hits.as_slice() {
        [one] => *one,
        [] => die(&format!("no repo under {} matches \"{q}\"", root.display())),
        many => {
            let names: Vec<_> = many.iter().map(|p| rel(p)).collect();
            die(&format!("\"{q}\" matches {}: {}", many.len(), names.join(", ")))
        }
    };
    if !open {
        return println!("{}", path.display());
    }
    let editor = git_config("garden.editor").unwrap_or_else(|| "code".into());
    if !launch(&editor, path).is_ok_and(|s| s.success()) {
        die(&format!("could not run {editor} {}", path.display()));
    }
}

/// Windows editors are often .cmd shims (code.cmd), which only cmd can run.
/// The path is quoted by hand so a folder name with `&` or `|` stays text; `%`
/// would still be expanded inside quotes, so such a path is refused.
#[cfg(windows)]
fn launch(editor: &str, path: &Path) -> std::io::Result<std::process::ExitStatus> {
    use std::os::windows::process::CommandExt;
    let path = path.to_string_lossy();
    if path.contains(['"', '%']) {
        return Err(std::io::ErrorKind::InvalidInput.into());
    }
    Command::new("cmd").args(["/S", "/C"]).raw_arg(format!("\"{editor} \"{path}\"\"")).status()
}

#[cfg(not(windows))]
fn launch(editor: &str, path: &Path) -> std::io::Result<std::process::ExitStatus> {
    Command::new(editor).arg(path).status()
}

fn config_root() -> Option<PathBuf> {
    git_config("garden.root").map(PathBuf::from).map(native)
}

/// `D:/Projects` from git config would print as `D:/Projects\owner\repo`.
fn native(p: PathBuf) -> PathBuf {
    PathBuf::from(p.to_string_lossy().replace('/', std::path::MAIN_SEPARATOR_STR))
}

pub fn git_config(key: &str) -> Option<String> {
    let out = Command::new("git").args(["config", "--global", key]).output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !s.is_empty()).then_some(s)
}

fn die(msg: &str) -> ! {
    eprintln!("gh garden: {msg}");
    exit(1)
}
