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
every `gh garden` run from the configured `garden.root` writes this machine's
unsaved / untracked / diverged repos into a secret gist named
`gh-garden state` (one file per machine), then shows what every other machine
last reported and how long ago. `gh garden snapshot` forces the note for any
root. A `--root` elsewhere (a test folder) never overwrites the note.

The gist holds repo names (private repos included), absolute local paths,
the machine name, branch names and counts, never file contents.
Secret gists are unlisted, not private: anyone with the URL can read it.
Running from the configured root writes the note on its own; set
`git config --global garden.sync false` to stop that (`snapshot` still writes
when you ask for it), and delete the gist with `gh gist delete` to remove
what was already written.

`--plan` single-quotes every path and branch name it puts in a command. A
value with a quote or control character gets no command and is listed under
"Needs you" instead. Origin URLs are printed without `user:password@`.

## Navigate

Local only, no GitHub call:

```
gh garden path qoraph          # D:\Projects\NaNIsaac\QoraPhase
cd (gh garden path qoraph)     # PowerShell; bash: cd "$(gh garden path qoraph)"
gh garden open codigest        # git config garden.editor, default `code`
gh garden path                 # every repo, one per line (pipe to fzf)
```

Exact folder name wins; otherwise a substring of `<owner>/<repo>`.
Several matches → it lists them and exits 1.

## Install

Windows or Linux, no Rust needed:

```
gh extension install NaNIsaac/gh-garden
git config --global garden.root D:/Projects
git config --global garden.machine lab
gh garden
```

Update: `gh extension upgrade garden`. Releases come from tags `v*`.

From source: `cargo build --release`, copy the binary to the repo root,
`gh extension install .`. Rebuild → copy again; `gh` runs the copy.
Run these in a clone of this repo.

<!-- test -->
```sh
cargo build --release
```

Copy the binary to the repo root.

<!-- test unix -->
```sh
cp target/release/gh-garden .
```

<!-- test windows -->
```powershell
Copy-Item target/release/gh-garden.exe .
```

<!-- test -->
```sh
gh extension install .
```

Check that `gh` finds it (exits non-zero if not):

<!-- test -->
```sh
gh garden --help
```
