# gh garden

What differs between your GitHub account and this machine.

```
gh garden reconcile [--root <dir>] [--json]
```

Lists every repo you own or can reach through an org, finds every git checkout
under `<root>`, matches them by `origin` and reports:

| Section | Meaning |
|---|---|
| Work not on GitHub | modified files, unpushed commits or stashes |
| Only untracked files | probably junk, maybe a forgotten new file |
| Diverged / Behind GitHub | default branch vs GitHub's head, no fetch needed |
| On GitHub, not cloned here | archived repos are skipped |
| Renamed on GitHub | `origin` still uses the old name |
| Not at `<root>/<owner>/<repo>` | folder name or owner folder is off |
| Local only | no origin, or origin outside your account |
| Not a git repo | stray folder inside an owner folder |

The last line answers the real question: **can I leave this machine?**

Read-only. Nothing is fetched, pulled, pushed, moved or deleted.

## Install

```
cargo build --release
copy target\release\gh-garden.exe .
gh extension install .
git config --global garden.root D:/Projects
```

Rebuild → copy the exe again; `gh` runs the copy in the repo root.
