//! What the snapshot gist must not carry: credentials, key files, measured data.
//!
//! A secret gist is unlisted, not private, so everything that is about to be written is checked
//! first. A hit stops the whole write; the caller says which rule and which repo, never the
//! matched text.

use crate::state::Snapshot;

#[derive(Debug, PartialEq, Eq)]
pub struct Hit {
    pub rule: &'static str,
    pub repo: Option<String>,
}

/// Rules that fire on `snap`, in order of the findings; empty means nothing to hold back.
pub fn scan(snap: &Snapshot) -> Vec<Hit> {
    let mut hits = Vec::new();
    let mut check = |repo: Option<&str>, text: &str| {
        for rule in rules(text) {
            let hit = Hit { rule, repo: repo.map(str::to_string) };
            if !hits.contains(&hit) {
                hits.push(hit);
            }
        }
    };
    check(None, &snap.machine);
    check(None, &snap.root);
    for f in &snap.findings {
        let repo = f.repo.as_deref();
        check(repo, &f.detail);
        if let Some(path) = &f.path {
            check(repo, &path.to_string_lossy());
        }
        if let Some(fix) = &f.fix {
            check(repo, fix);
        }
        if let Some(r) = repo {
            check(repo, r);
        }
    }
    hits
}

fn rules(text: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    if url_credentials(text) {
        out.push("credentials in a URL");
    }
    if token_like(text) {
        out.push("token or key");
    }
    if text.contains("-----BEGIN") && text.contains("PRIVATE KEY") {
        out.push("private key");
    }
    if assigned_secret(text) {
        out.push("password or token assignment");
    }
    if sensitive_file_name(text) {
        out.push("key or data file name");
    }
    out
}

/// `scheme://user:password@host` or `scheme://token@host`.
fn url_credentials(text: &str) -> bool {
    text.match_indices("://").any(|(i, _)| {
        let rest = &text[i + 3..];
        let authority = rest.split(['/', ' ', '"', '\'', '\\']).next().unwrap_or("");
        let Some((userinfo, _)) = authority.split_once('@') else { return false };
        match userinfo.split_once(':') {
            Some((_, password)) => !password.is_empty(),
            None => userinfo.len() >= 20 && userinfo.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        }
    })
}

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
}

fn token_like(text: &str) -> bool {
    words(text).any(|w| {
        let long = |prefix: &str, min: usize| w.starts_with(prefix) && w.len() >= min;
        ["ghp_", "gho_", "ghu_", "ghs_", "ghr_"].iter().any(|p| long(p, 24))
            || long("github_pat_", 30)
            || long("glpat-", 20)
            || long("sk-", 24)
            || long("AIza", 39)
            || (w.starts_with("AKIA") && w.len() == 20 && w[4..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()))
            || (w.starts_with("xox") && w.len() >= 12 && matches!(w.as_bytes().get(3), Some(b'b' | b'a' | b'p' | b'r' | b's')) && w.as_bytes().get(4) == Some(&b'-'))
    })
}

/// `password = hunter22`, `"token": "abc..."`, `api_key=...`; placeholders do not count.
fn assigned_secret(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    ["password", "passwd", "secret", "token", "api_key", "apikey", "api-key", "authorization"]
        .iter()
        .any(|key| {
            lower.match_indices(key).any(|(i, _)| {
                let after = lower[i + key.len()..].trim_start_matches(['"', '\'', ' ']);
                let Some(value) = after.strip_prefix([':', '=']) else { return false };
                let value = value.trim_start_matches(['"', '\'', ' ']);
                let value = value.split(['"', '\'', ' ', ',', ';']).next().unwrap_or("");
                value.len() >= 6 && !value.starts_with(['<', '$', '*', '{', '%']) && !matches!(value, "null" | "false" | "true" | "none")
            })
        })
}

const KEY_FILES: [&str; 8] = ["id_rsa", "id_ed25519", "id_ecdsa", ".netrc", ".npmrc", ".pypirc", "credentials", "credentials.json"];
const KEY_EXTENSIONS: [&str; 6] = ["pem", "key", "p12", "pfx", "kdbx", "ppk"];
/// Detector, spectrometer and array formats: a name like this in a published line means measured data.
const DATA_EXTENSIONS: [&str; 12] = ["h5", "hdf5", "hdf", "tif", "tiff", "dat", "spec", "raw", "npy", "npz", "mat", "cbf"];

fn sensitive_file_name(text: &str) -> bool {
    text.split(|c: char| c.is_whitespace() || matches!(c, '/' | '\\' | '"' | '\'' | ',' | '(' | ')')).any(|name| {
        let lower = name.to_ascii_lowercase();
        if KEY_FILES.contains(&lower.as_str()) || lower == ".env" || lower.starts_with(".env.") {
            return true;
        }
        lower
            .rsplit_once('.')
            .is_some_and(|(stem, ext)| !stem.is_empty() && (KEY_EXTENSIONS.contains(&ext) || DATA_EXTENSIONS.contains(&ext)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::{Finding, Status};
    use std::path::PathBuf;

    fn finding(repo: &str, path: &str, detail: &str, fix: Option<&str>) -> Finding {
        Finding {
            status: Status::Unsaved,
            repo: Some(repo.into()),
            path: Some(PathBuf::from(path)),
            detail: detail.into(),
            fix: fix.map(str::to_string),
        }
    }

    fn snapshot(findings: Vec<Finding>) -> Snapshot {
        Snapshot { machine: "lab".into(), root: "D:/03_Develop".into(), at: 0, findings }
    }

    #[test]
    fn an_ordinary_snapshot_passes() {
        let snap = snapshot(vec![
            finding(
                "NaNIsaac/codigest",
                "D:/03_Develop/NaNIsaac/codigest",
                "3 modified, 2 unpushed commits, 1 untracked (on fix/token-refresh)",
                Some("git -C 'D:/03_Develop/NaNIsaac/codigest' push origin fix/token-refresh"),
            ),
            finding("NaNIsaac/spec_manager", "D:/03_Develop/NaNIsaac/_wt/spec-manager-audit", "1 stashes (worktree on docs/audit-lineage)", None),
        ]);
        assert_eq!(scan(&snap), []);
    }

    #[test]
    fn credentials_in_a_url_are_found_and_the_value_is_not_reported() {
        let snap = snapshot(vec![finding("a/b", "D:/x", "origin https://bob:s3cr3t-pass@github.com/a/b.git", None)]);
        let hits = scan(&snap);
        assert_eq!(hits, [Hit { rule: "credentials in a URL", repo: Some("a/b".into()) }]);
        assert!(!format!("{hits:?}").contains("s3cr3t"));
        assert!(url_credentials("https://ghp_abcdefghijklmnopqrstuvwxyz@github.com/a/b"));
        assert!(!url_credentials("ssh://git@github.com/a/b.git") && !url_credentials("https://github.com/a/b"));
    }

    #[test]
    fn tokens_keys_and_assignments_are_found() {
        assert!(token_like("on ghp_0123456789abcdefghijklmnopqrstuvwxyz"));
        assert!(token_like("github_pat_11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz"));
        assert!(token_like("AKIAIOSFODNN7EXAMPLE") && token_like("xoxb-123456789012-abcdef"));
        assert!(!token_like("fix/ghp_ and sk-short and AKIAshort"));
        assert!(rules("-----BEGIN OPENSSH PRIVATE KEY-----").contains(&"private key"));
        assert!(assigned_secret("password = hunter22xyz") && assigned_secret(r#"{"api_key": "abcdef123456"}"#));
        assert!(!assigned_secret("token: <redacted>") && !assigned_secret("fix token refresh") && !assigned_secret("password: null"));
    }

    #[test]
    fn key_files_and_measured_data_names_are_found() {
        for name in [".env", "D:/x/.env.local", "~/.ssh/id_rsa", "keys/server.pem", "scan_244.dat", "frames/run1.h5", "x.TIFF", "credentials.json"] {
            assert!(sensitive_file_name(name), "{name}");
        }
        for name in ["main", "data", "dat", "readme.md", "src/lib.rs", "environment", "fix/key-rotation"] {
            assert!(!sensitive_file_name(name), "{name}");
        }
    }

    #[test]
    fn every_place_the_snapshot_publishes_is_checked() {
        let mut snap = snapshot(vec![finding("a/b", "D:/x", "1 modified (on main)", None)]);
        assert!(scan(&snap).is_empty());
        snap.findings[0].fix = Some("git -C D:/x push origin ghp_0123456789abcdefghijklmnopqrstuvwxyz".into());
        assert_eq!(scan(&snap).len(), 1);
        snap.findings[0].fix = None;
        snap.findings[0].path = Some(PathBuf::from("D:/data/run1.h5"));
        assert_eq!(scan(&snap)[0].rule, "key or data file name");
        snap.findings[0].path = None;
        snap.machine = "https://u:p4ssw0rd@host".into();
        assert_eq!(scan(&snap)[0].repo, None);
    }
}
