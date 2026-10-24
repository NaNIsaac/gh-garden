# gh garden

[![Release](https://img.shields.io/github/v/release/NaNIsaac/gh-garden)](https://github.com/NaNIsaac/gh-garden/releases) [![CI](https://github.com/NaNIsaac/gh-garden/actions/workflows/readme.yml/badge.svg)](https://github.com/NaNIsaac/gh-garden/actions/workflows/readme.yml) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

**See what differs between your GitHub account and the machines you work on: unsaved work, repos you never cloned, folders in the wrong place.**

[한국어](README.ko.md)

`gh garden` is a [GitHub CLI](https://cli.github.com/) extension for people who keep many
repositories on more than one computer. It lists the repositories your account can reach, finds
every git checkout under one folder on this machine, matches them by `origin`, and tells you where
the two sides disagree.

The question it exists to answer is the one before you close the laptop: **can I leave this
machine?** If a repo here holds edits, commits or stashes that GitHub does not have, the answer is
no, and the exit code says so.

## What it does

- **Finds work that exists only here.** Modified files, unpushed commits and stashes, per repo.
- **Finds what is missing here.** Repos on GitHub that this machine never cloned, with their size.
- **Finds drift.** A local branch behind or diverged from GitHub's default branch, an `origin`
  that still points at an old name, a folder that is not at `<root>/<owner>/<repo>`.
- **Prints the fix, never runs it.** `--plan` writes the `git` and `gh` commands for each finding.
  You read them, then run the ones you want.
- **Talks to scripts.** `--json` and the exit code are made for hooks and scripts.
- **Jumps to a repo.** `gh garden path qora` and `gh garden open qora` need no GitHub call.
- **Optionally remembers other machines.** A machine you switched off can leave a note in a secret
  gist, written only when you ask and checked for credentials first.

It never changes a repository: no fetch, no checkout, no commit. Everything it knows about GitHub
comes through the `gh` CLI you are already signed in with.

## Install

You need `git` and [`gh`](https://cli.github.com/) on `PATH`, and `gh auth login` done.
Windows (x64) or Linux (x64), no Rust needed:

```
gh extension install NaNIsaac/gh-garden
git config --global garden.root D:/Projects
git config --global garden.machine lab
gh garden
```

`garden.root` is the folder that holds your clones, laid out as `<root>/<owner>/<repo>`.
`garden.machine` is the name other machines see. Update with `gh extension upgrade garden`;
releases come from tags `v*`.

From source: `cargo build --release`, copy the binary to the repo root, `gh extension install .`.
Rebuild → copy again; `gh` runs the copy. Run these in a clone of this repo.

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

Check that `gh` finds it (exits non-zero if not). `path` makes no GitHub call, so this also works
without a login; with `--root .` it lists the clone you are standing in:

<!-- test -->
```sh
gh garden --help
```

<!-- test console -->
```console
$ gh garden path --root .
.
```

## 1. Before you leave: is it safe to power this machine off?

```
gh garden
```

```console
root  D:\Projects  (lab)

Work not on GitHub (2)
  NaNIsaac/clippath                    NaNIsaac\clippath            1 modified (on main)
  NaNIsaac/deai                        NaNIsaac\deai                1 unpushed commits (on main)
Only untracked files (1)
  NaNIsaac/roi_rectangle               NaNIsaac\roi_rectangle       1 untracked (on main)
Behind GitHub (1)
  NaNIsaac/hanfix                      NaNIsaac\hanfix              main: 1 behind
On GitHub, not cloned here (66)
  NaNIsaac/cohere                                                   fork, 1 MB
  NaNIsaac/cpython                                                  fork, 634 MB
  ...
Not at <root>/<owner>/<repo> (1)
  NaNIsaac/eml                         NaNIsaac\eml-old             expected NaNIsaac\eml
Local only (1)
  -                                    NaNIsaac\sketch              no origin remote
Not a git repo (under an owner folder) (1)
  -                                    NaNIsaac\scratch-notes

Synced: 1
NOT safe to leave this machine: 2 repo(s) have work that isn't on GitHub.
69 finding(s) have a fix: gh garden reconcile --plan
```

The sample is from a scratch folder, shortened where marked. Sections appear in this order, and a
section with nothing in it is left out. The verdict at the end is one of three lines:

- `Safe to leave this machine: everything here is on GitHub.`
- `Safe to leave this machine, unless N repo(s) hold untracked files you need.` Untracked files are
  often build junk and sometimes a forgotten new file; only you know which.
- `NOT safe to leave this machine: N repo(s) have work that isn't on GitHub.` The exit code is 2.

Everything else (behind, not cloned, misplaced) is housekeeping and does not change the verdict.

To make the check automatic, run it from a logoff or shutdown hook and act on the exit code:

```sh
gh garden > /dev/null 2>&1
[ $? -eq 2 ] && echo "unsaved work: run gh garden"
```

```powershell
gh garden *> $null
if ($LASTEXITCODE -eq 2) { Write-Warning 'unsaved work: run gh garden' }
```

A repo that has several `git worktree` checkouts is counted once for its branches, commits and
stashes (the main checkout reports those); a worktree reports only its own modified and untracked
files.

## 2. Set up a new PC: clone everything

On a fresh machine every repo is "On GitHub, not cloned here". Set the root and ask for the plan:

```
gh extension install NaNIsaac/gh-garden
git config --global garden.root D:/Projects
gh garden --plan
```

```
# For bash or PowerShell, not cmd.exe: arguments are single-quoted.
# On GitHub, not cloned here: NaNIsaac/QoraGeom 325 KB
gh repo clone 'NaNIsaac/QoraGeom' 'D:/Projects/NaNIsaac/QoraGeom'

# On GitHub, not cloned here: NaNIsaac/xarray fork, 53 MB
gh repo clone 'NaNIsaac/xarray' 'D:/Projects/NaNIsaac/xarray'
...
```

`--plan` only prints. Read it, then run what you want. Forks can be large (a `fork, 634 MB` line is
the size GitHub reports), so skip them by filtering the JSON instead of the text
(PowerShell 7; Windows PowerShell 5.1 hands `ConvertFrom-Json` the array as one object):

```powershell
gh garden --json | ConvertFrom-Json |
  Where-Object { $_.status -eq 'not_cloned' -and $_.detail -notlike 'fork*' } |
  ForEach-Object { Invoke-Expression $_.fix }
```

```sh
gh garden --json | jq -r '.[] | select(.status == "not_cloned" and (.detail | startswith("fork") | not)) | .fix' | sh
```

Archived repos are never listed. Clones land at `<root>/<owner>/<repo>`, which is where later runs
expect them.

## 3. Clean up after a rename

When a repo is renamed on GitHub, the old name keeps redirecting, so everything works until you
look closely: `origin` still holds the old name and the folder still has the old name. `gh garden`
asks GitHub what each unknown `origin` is called now and reports both:

```
Renamed on GitHub, origin is stale (1)
  NaNIsaac/new-name                    NaNIsaac\old-name            origin still says NaNIsaac/old-name
Not at <root>/<owner>/<repo> (1)
  NaNIsaac/new-name                    NaNIsaac\old-name            expected NaNIsaac\new-name
```

```
gh garden --plan
```

```
# Renamed on GitHub, origin is stale: NaNIsaac/new-name origin still says NaNIsaac/old-name
git -C 'D:/Projects/NaNIsaac/old-name' remote set-url origin 'https://github.com/NaNIsaac/new-name.git'

# Not at <root>/<owner>/<repo>: NaNIsaac/new-name expected NaNIsaac\new-name
mv 'D:/Projects/NaNIsaac/old-name' 'D:/Projects/NaNIsaac/new-name'
```

Run them in the order printed: the `mv` comes last, so the `set-url` line still finds the folder.
The `mv` is only offered when the target does not exist. The new `origin` is written as an
`https://` URL, even if the old one was `ssh`.

The same "not at the expected path" finding covers a repo cloned into the wrong owner folder or
under a different folder name. A repo whose `origin` is on GitHub but not in your account (someone
else's, or a repo that no longer exists) is listed under "Local only" and gets no command.

## 4. Script it: `--plan`, `--json`, exit codes

| Flag | Output | Notes |
| --- | --- | --- |
| (none) | The report above | Also shows other machines' notes |
| `--plan` | Commands, one per finding | Grouped and commented; never run |
| `--json` | One array of findings | All sections, including `synced` |

Both `--plan` and `--json` leave the GitHub note alone (neither reads nor writes it) and still exit
2 when work here is unsaved. A finding in the JSON looks like this; `fix` is absent when there is
no safe command and `path` is `null` for a repo that is not cloned:

```json
{
  "status": "unsaved",
  "repo": "NaNIsaac/deai",
  "path": "D:\\Projects\\NaNIsaac\\deai",
  "detail": "1 unpushed commits (on main)",
  "fix": "git -C 'D:/Projects/NaNIsaac/deai' push origin 'main'"
}
```

`status` is one of `unsaved`, `untracked`, `diverged`, `behind`, `not_cloned`, `renamed`,
`misplaced`, `local_only`, `not_git`, `synced` (see [Sections](#sections)). Findings are sorted by
that order, then repo, then path.

```powershell
# repos with work that exists only here
gh garden --json | ConvertFrom-Json | Where-Object status -eq 'unsaved' | ForEach-Object repo
```

```sh
gh garden --json | jq -r '.[] | select(.status == "unsaved") | .repo'
```

Exit codes are in the [reference](#exit-codes). Because the exit code is 2 when something is
unsaved, a script that must keep going under `set -e` needs `|| true` after `gh garden`.

`--plan` single-quotes every path and branch name, so the lines work in bash and PowerShell. They
are not meant for `cmd.exe`, where single quotes are not quotes and `&` ends the command. A value
that contains a quote or a control character gets no command and is listed under "Needs you"
instead. Origin URLs are printed without `user:password@`.

## 5. Jump to a repo

Local only, no GitHub call, fast enough for a prompt or an alias:

```
gh garden path qoraph          # D:\Projects\NaNIsaac\QoraPhase
cd (gh garden path qoraph)     # PowerShell; bash: cd "$(gh garden path qoraph)"
gh garden open codigest        # git config garden.editor, default `code`
gh garden path                 # every repo, one per line (pipe to fzf)
```

An exact folder name wins; otherwise any repo whose `<owner>/<repo>` contains the text matches
(case does not matter). Several matches → it lists them and exits 1. `open` starts the editor with
the path as one argument, also on Windows where editors are often `.cmd` shims; a path or editor
value containing `"` or `%` is refused there.

## 6. See what your other machines hold

A switched-off machine cannot be asked. So a machine can leave a note, only when you ask for it:
`gh garden snapshot` writes this machine's unsaved, untracked and diverged repos into a secret gist
named `gh-garden state` (one file per machine) for any root, and
`git config --global garden.sync true` makes every `gh garden` run from the configured
`garden.root` do the same. Without either, nothing is written to GitHub. A `--root` elsewhere (a
test folder) never overwrites the note unless you run `snapshot`.

Every normal run reads the note and shows what the other machines last reported and how long ago:

```
On lab (D:\Projects, 3h ago):
  NaNIsaac/deai                        1 unpushed commits (on main)
```

If no other machine has written a note, the report ends with "No other machine has reported." A
machine name is compared without regard to case, and a machine never sees itself in that list. [Privacy and safety](#privacy-and-safety) says what the note holds and how it is
checked.

## Reference

- [Commands](#commands)
- [Sections](#sections)
- [Exit codes](#exit-codes)
- [Configuration](#configuration)
- [Privacy and safety](#privacy-and-safety)
- [Upgrading from 0.3](#upgrading-from-03)
- [How it works](#how-it-works)
- [Development](#development)

### Commands

```
gh garden [reconcile] [--root <dir>] [--plan] [--json]
gh garden snapshot    [--root <dir>]
gh garden path [<query>] [--root <dir>]
gh garden open <query>   [--root <dir>]
```

| Command | Does | Reads GitHub | Writes GitHub |
| --- | --- | --- | --- |
| `reconcile` | Report (`status` is an alias; the default) | yes | only with `garden.sync` |
| `snapshot` | `reconcile`, then leave the note | yes | the note |
| `path` | Print the matching repo's folder | no | no |
| `open` | Open it in `garden.editor` | no | no |

`--root <dir>` overrides `garden.root` for one run. `-h`, `--help` or `help` prints the usage.
Options and words can come in any order. There is no `--version`: `gh extension list` shows it.

### Sections

| Section | `status` | `--plan` command |
| --- | --- | --- |
| Work not on GitHub | `unsaved` | `push`, only if safe |
| Only untracked files | `untracked` | none |
| Diverged from GitHub | `diverged` | none |
| Behind GitHub | `behind` | `pull --ff-only` or `fetch` |
| On GitHub, not cloned here | `not_cloned` | `gh repo clone` |
| Renamed on GitHub, origin is stale | `renamed` | `remote set-url` |
| Not at `<root>/<owner>/<repo>` | `misplaced` | `mv`, if the target is free |
| Local only | `local_only` | none |
| Not a git repo (under an owner folder) | `not_git` | none |
| Synced | `synced` | none |

How each is decided, in the order a repo is checked:

- **Which repos.** Every folder with a `.git` (a directory or a worktree file) up to four levels
  below the root, not descending into a repo. `node_modules`, `target`, `.venv`, `venv` and
  `__pycache__` are skipped. GitHub's side is every repository your account owns or reaches as an
  organization member, private and forks included; repos of other owners you are only a
  collaborator on are not listed.
- **Unsaved.** At least one of: a modified or staged file, a commit on any local branch that no
  remote-tracking ref contains, a stash. Untracked files are added to the detail when there is
  already something unsaved; alone they make the repo `untracked`. Ignored files are invisible.
- **Behind and diverged.** The local branch with the name of GitHub's default branch is compared
  with the commit GitHub reports for it, without fetching. If that commit is not in the local object
  store at all, GitHub has commits never fetched here: `behind`. Behind and ahead at once is
  `diverged`. A repo without that local branch is not compared.
- **Plan for unsaved.** `git push origin <branch>` only when nothing is modified or stashed, there
  are unpushed commits, the repo is not behind GitHub and `HEAD` is on a branch. Edits, stashes and
  a push GitHub would reject get no command: they need you.
- **Plan for behind.** `git pull --ff-only` when the checkout is on the default branch with a clean
  tree, otherwise `git fetch origin`, which touches no work.
- **Renamed.** An `origin` the account listing does not know is looked up with `gh api repos/...`.
  If GitHub redirects it to a repo in your list, that is `renamed`. A redirect to a repo outside
  your account is `local_only` ("not in your account"); a lookup that fails is `local_only`
  ("origin repo not found on GitHub").
- **Local only.** No `origin`, an `origin` that is not on `github.com`, or one of the two cases
  above.
- **Not a git repo.** A folder directly under `<root>/<owner>` for an owner of your repos, with no
  `.git` and no repo inside it. Folders under an owner that your account does not have are not
  looked at.
- **Not cloned.** Archived repos are skipped; the detail is the size GitHub reports, `fork, ` first
  for a fork.

### Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Done, and nothing here is unsaved. Also: stdout closed early (`gh garden \| head`) |
| 1 | Could not run: no root, root is not a folder, `gh` failed or is not signed in, unknown argument or command, `path` with no or several matches, `open` failed |
| 2 | Report printed, and at least one repo has work that GitHub does not have |

Exit 2 applies to `reconcile`, `snapshot`, `--plan` and `--json` alike. Untracked-only repos and
every other section leave it at 0.

### Configuration

All keys are read from the global git config (`git config --global`); a repo-level value is not
used.

| Key | Default | Effect |
| --- | --- | --- |
| `garden.root` | none | Folder that holds `<owner>/<repo>`; `D:/Projects` is fine on Windows |
| `garden.machine` | host name | Name on the note |
| `garden.sync` | off | `true`: a run from `garden.root` leaves the note |
| `garden.editor` | `code` | Program `open` starts |
| `garden.log` | state folder | File the note log is appended to |

The machine name is the first of `$GARDEN_MACHINE`, `garden.machine`, `$COMPUTERNAME` and
`$HOSTNAME`, else `unknown`. bash does not export `HOSTNAME`, so on Linux set `garden.machine`.
The log is `%LOCALAPPDATA%\gh-garden\log.jsonl`, or `$XDG_STATE_HOME/gh-garden/log.jsonl`, or
`~/.local/state/gh-garden/log.jsonl`.

### Privacy and safety

- **Read-only on your repos.** No fetch, checkout, commit or write. `--plan` prints commands and
  nothing else.
- **GitHub access is `gh`.** Authentication, hosts and tokens are whatever `gh auth status` says.
  Only `github.com` remotes are recognized.
- **Nothing is uploaded by default.** The note is written by `snapshot`, or by any run from
  `garden.root` when `garden.sync` is `true`.
- **What the note holds.** Repo names (private repos included), absolute local paths, the machine
  name, branch names and counts, never file contents. Secret gists are unlisted, not private:
  anyone with the URL can read it, so treat the note as published.
- **What is checked first.** Before every write the note is checked for credentials (a
  `user:password@` URL, GitHub, AWS, Slack and similar tokens, `password=` style assignments,
  private key headers), for key file names (`.env`, `id_rsa`, `*.pem`, `*.key`, ...) and for
  measured-data file names (`*.h5`, `*.tif`, `*.dat`, `*.spec`, `*.npy`, ...) in any repo name,
  path, branch name or command it would carry. A hit stops the write, says which rule and which repo
  (never the matched text), and the run goes on read-only.
- **The log.** `published`, `scan_blocked` and `publish_failed` are logged apart, one JSON line each,
  naming rules and repos, never matched text.
- **Taking it back.** `gh gist delete` removes a note that was already written.
- These checks catch common formats, not every possible secret.
- **Credentials in `origin`.** `user:password@` is dropped from every URL before it is printed,
  saved or put in a command. It does not cover a password containing an unencoded `/`, `?` or `#`,
  a token in a query string, or the scp-like `user:password@host:path` form.

### Upgrading from 0.3

Release v0.3.0 wrote the note on every run from the configured root. Later versions write nothing
unless you run `snapshot` or set `garden.sync true`, and check the note first. If you relied on the
old behavior, `git config --global garden.sync true` brings it back. A note written by 0.3 stays in
your gist until you run `gh gist delete`.

### How it works

- **GitHub side.** One paginated GraphQL call through `gh api graphql` for the repositories, and
  `gh api repos/<id>` for each `origin` the listing does not know.
- **Local side.** `git status --porcelain=v2 --branch`, `git rev-list` for unpushed commits and
  stashes, and `git rev-list --left-right --count` against GitHub's head commit. One thread per repo.
- **The note.** One secret gist with one JSON file per machine; the write response carries every
  file, so a run costs two `gh` calls. Last write wins, which is enough for a handful of machines
  owned by one person. A file that was edited into garbage is ignored, not fatal.
- **Not covered.** Submodules, non-GitHub hosts and GitHub Enterprise, repos you only collaborate
  on, and branches other than the default one when comparing with GitHub.

### Development

```sh
git clone https://github.com/NaNIsaac/gh-garden
cd gh-garden
cargo test
cargo build --release
```

`FEEDBACK.md` is the dogfooding log: what each version found on a real machine and what changed
because of it.

## License

MIT. See [LICENSE](LICENSE).
