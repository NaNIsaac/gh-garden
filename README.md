# gh garden

What differs between your GitHub account and your machines.

```
gh garden reconcile [--root <dir>] [--plan] [--json]
gh garden snapshot  [--root <dir>]
```

Lists every repo you own or can reach through an org, finds every git checkout
under `<root>`, matches them by `origin` and reports:

| Section | Meaning | `--plan` |
|---|---|---|
| Work not on GitHub | modified files, unpushed commits or stashes | `push`, only if clean and not behind |
| Only untracked files | probably junk, maybe a forgotten new file | — |
| Diverged / Behind GitHub | default branch vs GitHub's head, no fetch needed | `pull --ff-only` if clean |
| On GitHub, not cloned here | archived repos are skipped | `gh repo clone` to `<root>/<owner>/<repo>` |
| Renamed on GitHub | `origin` still uses the old name | `remote set-url` |
| Not at `<root>/<owner>/<repo>` | folder name or owner folder is off | `mv`, if the target is free |
| Local only | no origin, or origin outside your account | — |
| Not a git repo | stray folder inside an owner folder | — |

The last line answers the real question: **can I leave this machine?**
Exit code 2 when the answer is no.

`reconcile` is read-only. `--plan` prints commands; it never runs them.

## Several machines

A switched-off machine can't be asked. So each machine leaves a note:

```
gh garden snapshot
```

writes this machine's unsaved / untracked / diverged repos into a secret gist
named `gh-garden state` (one file per machine). `reconcile` on any other
machine then ends with what each one last reported, and how long ago.

The gist holds repo names, paths and counts, never file contents.
Secret gists are unlisted, not private: anyone with the URL can read it.

## Install

```
cargo build --release
copy target\release\gh-garden.exe .
gh extension install .
git config --global garden.root D:/Projects
git config --global garden.machine lab
```

Rebuild → copy the exe again; `gh` runs the copy in the repo root.
