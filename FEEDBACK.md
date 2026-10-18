# Dogfooding log

## 2026-10-18 — first run, home PC (`D:/03_Develop`, 52 repos)

Every finding checked by hand against `git status` / `gh`. No false positives.

**Real findings**
- QoraGeom: 2 unpushed commits → pushed (fast-forward, fetched first).
- clippath, deai, roi_rectangle: uncommitted edits. Left alone: live work.
- ponyhead-final: folder ≠ repo `ponyhead`. Not moved yet (owner's call).
- eclass-ta: `git init` with zero commits, no origin.
- `*-review` folders: screenshots, not repos.
- cpython fork: on GitHub, never cloned here, pushed today → some other
  machine is working on it. This is the cross-machine question v0.1 can't answer.
- `git fetch` on all 53 repos: nothing behind. GitHub had nothing this PC lacked.

**Fixed after this run**
- Untracked-only repos (`.claude/`, `p.out`) were in "NOT safe" → own section.
- `git worktree` checkouts reported Behind + Misplaced twice → only their own files now.
- Expected path printed absolute with mixed `/` `\` → relative to root.
- `gh garden` ran a stale exe after rebuild (extension = copied exe). README note.

**Scenario fixtures** (scratch root, cloned + rewound): behind, diverged,
wrong owner folder, worktree, stash-only, detached HEAD, deleted origin,
someone else's repo via redirect (`github/hub` → `mislav/hub`). All correct.
Not covered: in-account rename (no renamed repo to test with).

**Next, by pain felt**
1. Machine snapshot (private gist) so the laptop sees lab-PC unsaved work.
2. `--plan`: print the fix commands (clone, set-url, rename folder), run nothing.
3. Exit code ≠ 0 when not safe, for a shutdown/logoff hook.

Acted on: ponyhead-final → ponyhead (folder rename, owner approved).

## 2026-10-18 — v0.2: `--plan`, `snapshot`, exit 2

All three from the list above shipped.

**Trial and error**
- `--plan` first offered `git push` for a *diverged* repo — GitHub would
  reject it. Push is now only offered when clean, stash-free and not behind.
- Deleting the last test file from the gist → 422 (a gist can't be empty).
  Harmless now that the real machine's file lives there; worth knowing.
- Hostname `DESKTOP-SG0F0O8` is unreadable in "On <machine>" → `garden.machine`.
- `GARDEN_MACHINE=` (empty) wrote a file named `.json` to the gist and showed
  up as a phantom machine. Empty names now fall through to config/hostname.
- `gh garden reconcile | head` panicked on the closed pipe → exits quietly.
- This PC is the home PC (`garden.machine = home`); the lab PC is separate
  and has not run garden yet.
- Cross-machine path verified with a fake machine (`GARDEN_MACHINE=TEST-LAPTOP`
  on the scratch fixtures): this PC printed its 2 unsaved repos. Test file removed.

**Open questions from real use**
- `--plan` offers `gh repo clone NaNIsaac/cpython`: a fork of a huge repo,
  pushed today from somewhere else. Clone plan should probably say size, or
  forks should be opt-in.
- Snapshots go stale silently. Only `snapshot` writes them, so it needs a
  habit or a hook (logoff task running `gh garden snapshot`).
- 4 repos hold uncommitted edits. The tool can see them; only the owner
  can decide to commit, stash or drop.
