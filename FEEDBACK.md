# Dogfooding log

## 2026-10-18 — first run, lab PC (`D:/03_Develop`, 52 repos)

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
