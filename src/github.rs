//! GitHub side. Everything goes through the `gh` CLI, so auth, hosts and
//! tokens are whatever `gh auth status` already says.

use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub name_with_owner: String,
    pub is_fork: bool,
    pub is_archived: bool,
    /// Kilobytes, as GitHub reports it.
    pub disk_usage: Option<u64>,
    pub default_branch_ref: Option<BranchRef>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BranchRef {
    pub name: String,
    pub target: Target,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Target {
    pub oid: String,
}

impl Repo {
    pub fn owner(&self) -> &str {
        self.name_with_owner.split('/').next().unwrap_or("")
    }
    pub fn name(&self) -> &str {
        self.name_with_owner.split('/').nth(1).unwrap_or("")
    }
}

const QUERY: &str = r#"
query($endCursor: String) {
  viewer {
    repositories(first: 100, after: $endCursor, ownerAffiliations: [OWNER, ORGANIZATION_MEMBER]) {
      nodes {
        nameWithOwner isFork isArchived diskUsage
        defaultBranchRef { name target { oid } }
      }
      pageInfo { hasNextPage endCursor }
    }
  }
}"#;

/// Every repository the signed-in account owns or belongs to via an org.
pub fn list_repos() -> Result<Vec<Repo>, String> {
    let out = Command::new("gh")
        .args(["api", "graphql", "--paginate", "-f"])
        .arg(format!("query={QUERY}"))
        .args(["--jq", ".data.viewer.repositories.nodes[]"])
        .output()
        .map_err(|e| format!("could not run gh: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("bad gh output: {e}")))
        .collect()
}

/// What GitHub currently calls `owner/repo`. Renamed or transferred repos
/// redirect, so this returns the new name; `None` if it is gone or private
/// to someone else.
pub fn canonical_name(id: &str) -> Option<String> {
    let out = Command::new("gh")
        .args(["api", &format!("repos/{id}"), "--jq", ".full_name"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
}
