//! Per-machine snapshots in one secret gist, so a machine that is switched
//! off can still tell the others what it was holding.
//! ponytail: one gist, one file per machine, last write wins; enough for a
//! handful of machines owned by one person.

use crate::reconcile::{Finding, Status};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const DESCRIPTION: &str = "gh-garden state";

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub machine: String,
    pub root: String,
    /// Unix seconds.
    pub at: u64,
    pub findings: Vec<Finding>,
}

/// Only what lives on this machine alone: nothing GitHub can tell the others.
pub fn keep(status: Status) -> bool {
    matches!(status, Status::Unsaved | Status::Untracked | Status::Diverged)
}

pub fn machine() -> String {
    let env = |k| std::env::var(k).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    env("GARDEN_MACHINE")
        .or_else(|| crate::git_config("garden.machine"))
        .or_else(|| env("COMPUTERNAME"))
        .or_else(|| env("HOSTNAME"))
        .unwrap_or_else(|| "unknown".into())
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Write `snap` (if any) and return every machine's snapshot, in two gh calls:
/// the write's response already carries all files.
pub fn sync(snap: Option<&Snapshot>) -> Result<Vec<Snapshot>, String> {
    const JQ: &str = "[.files[].content]";
    let files = snap.map(|s| {
        let content = serde_json::to_string_pretty(s).unwrap();
        serde_json::json!({ format!("{}.json", s.machine): { "content": content } })
    });
    let raw = match (gist_id()?, files) {
        (Some(id), Some(files)) => gh(
            &["api", &format!("gists/{id}"), "-X", "PATCH", "--input", "-", "--jq", JQ],
            &serde_json::json!({ "files": files }),
        )?,
        (Some(id), None) => gh(&["api", &format!("gists/{id}"), "--jq", JQ], &serde_json::Value::Null)?,
        (None, Some(files)) => gh(
            &["api", "gists", "-X", "POST", "--input", "-", "--jq", JQ],
            &serde_json::json!({ "description": DESCRIPTION, "public": false, "files": files }),
        )?,
        (None, None) => return Ok(Vec::new()),
    };
    let contents: Vec<String> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    // A file someone hand-edited into garbage shouldn't hide the others.
    Ok(contents.iter().filter_map(|c| serde_json::from_str(c).ok()).collect())
}

fn gist_id() -> Result<Option<String>, String> {
    let jq = format!(r#".[] | select(.description == "{DESCRIPTION}") | .id"#);
    let out = gh(&["api", "gists", "--paginate", "--jq", &jq], &serde_json::Value::Null)?;
    Ok(out.lines().next().map(str::to_string))
}

fn gh(args: &[&str], body: &serde_json::Value) -> Result<String, String> {
    let mut child = Command::new("gh")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run gh: {e}"))?;
    if !body.is_null() {
        child.stdin.take().unwrap().write_all(body.to_string().as_bytes()).map_err(|e| e.to_string())?;
    }
    drop(child.stdin.take());
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Where `log` appends: `garden.log` from git config, else the user's state folder.
fn log_path() -> Option<std::path::PathBuf> {
    if let Some(p) = crate::git_config("garden.log") {
        return Some(p.into());
    }
    let var = |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(std::path::PathBuf::from);
    let base = var("LOCALAPPDATA").or_else(|| var("XDG_STATE_HOME")).or_else(|| var("HOME").map(|h| h.join(".local").join("state")))?;
    Some(base.join("gh-garden").join("log.jsonl"))
}

/// One JSON line per publish decision: `published`, `scan_blocked` (the check held it back) or
/// `publish_failed` (gh did not write it). Details name rules and repos, never matched text.
/// A log that cannot be written is not an error worth stopping for.
pub fn log(event: &str, detail: &str) {
    let Some(path) = log_path() else { return };
    let line = serde_json::json!({ "at": now(), "machine": machine(), "event": event, "detail": detail });
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{line}");
    }
}
pub fn ago(at: u64) -> String {
    let s = now().saturating_sub(at);
    match s {
        0..=89 => "just now".into(),
        90..=5399 => format!("{}m ago", s / 60),
        5400..=129_599 => format!("{}h ago", s / 3600),
        _ => format!("{}d ago", s / 86400),
    }
}
