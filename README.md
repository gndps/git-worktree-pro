# git-worktree-pro

A comprehensive git worktree management toolkit, available as a single binary (`gwtp`).

## Install

```bash
brew install gndps/tap/git-worktree-pro
```

`gwtp` has no shell aliases bundled — invoke it directly, or define your own
aliases/functions for the commands you use most.

## Commands

| Command | Description |
|---------|-------------|
| `gwtp list [--all] [--status]` | List worktrees |
| `gwtp list-detail [--all]` | List with full git status |
| `gwtp list-log [--all]` | List with recent commits |
| `gwtp tree [count] [--all]` | Tree view with commit history |
| `gwtp add <branch> [--from <src>]` | Create worktree at `~/.worktrees/<branch>/<repo>` |
| `gwtp add-random` | Create worktree with random suffix |
| `gwtp cd <wt>` | Navigate to worktree (prints `cd '<path>'` for eval) |
| `gwtp open <wt>` | Open worktree in editor |
| `gwtp open-pick [--all]` | Pick worktree with fzf |
| `gwtp copy-name <N>` | Copy folder name to clipboard |
| `gwtp rename <wt> <name>` | Rename worktree directory |
| `gwtp remove <wt> [--force]` | Remove worktree |
| `gwtp diff <P> <C>` | Diff committed changes between worktrees |
| `gwtp diff-code <P> <C>` | Open diff in editor |
| `gwtp diff-list <P> <C>` | Numstat diff |
| `gwtp diff-all <P> <C>` | Working dir diff |
| `gwtp note [text]` | Get/set worktree note |
| `gwtp slug [name]` | Get/set repo slug |
| `gwtp status` | Compact git status |
| `gwtp config <list\|set-hidden-wt\|set-hidden-br\|set-editor\|edit>` | General config |
| `gwtp sideload <subcommand>` | Sideloaded file management — see below |

### Sideload

"Sideload" is how `gwtp` keeps untracked, per-worktree files (`.env*`,
local config, etc.) in sync across worktrees. Which files are managed is
controlled by a standalone JSON file at
`<git-common-dir>/sideload_patterns.json`, with two gitignore-syntax
pattern lists:

```json
{
  "sideload_and_ignore": ["local/", "config/local.json"],
  "sideload_only": ["visible_dir/"]
}
```

Both lists are sideloaded (copied between worktrees) the same way. The
difference is `sideload_and_ignore` patterns are also mirrored into a
MANAGED BLOCK inside `.git/info/exclude`, so git treats them as ignored;
`sideload_only` patterns are left out of `.git/info/exclude`, so matched
files still show up in `git status` — useful when you want a file synced
across worktrees without hiding it from git. As in `.gitignore`, a
directory pattern (`local/`) covers every file beneath it, not just files
directly inside it.

| Command | Description |
|---------|-------------|
| `gwtp sideload init [--all]` | Sideload into all visible worktrees from main root |
| `gwtp sideload base [--from <spec>] [paths...]` | Broadcast sideload files from a source worktree to all others |
| `gwtp sideload cp-from <wt>` | Copy sideload files from worktree into current |
| `gwtp sideload cp-to <wt>` | Copy sideload files from current into worktree |
| `gwtp sideload add <pattern> [--only]` | Add a pattern (`--only` adds to `sideload_only` instead of `sideload_and_ignore`) |
| `gwtp sideload rm <pattern>` | Remove a pattern (from either list) |
| `gwtp sideload list-patterns` (alias `patterns`) | Show configured patterns, by list |
| `gwtp sideload edit` | Open `sideload_patterns.json` in your configured editor |
| `gwtp sideload exclude` | Explicitly re-sync `.git/info/exclude`'s managed block from `sideload_and_ignore` |
| `gwtp sideload list` (`-l` / `l` / `--list`) | Tree of sideloaded files in the current worktree, with each file's last-modified date |
| `gwtp sideload list-all` (`-la` / `la` / `--list-all`) | Global tree of sideloaded files across every worktree — each unique path shows a 6-char content hash, the date that content was last modified, and the worktree indices holding that version, so you can spot divergence before basing/copying files between worktrees |

`list-all` lists *every* worktree index for every (path, hash) entry — not
just the ones matching that version — colored by how each worktree relates
to it:

- plain: that worktree has this exact content
- red: that worktree doesn't have this path at all
- yellow: that worktree has a different, *older* version of this path
- green: that worktree has a different, *newer* version of this path

e.g. `local.json (de5b1c, 2026-06-20 10:00) [1,2,3,4,5]` with worktree 3
in red and 4 in green means worktrees 1, 2, and 5 have this exact version,
worktree 3 doesn't have the file at all, and worktree 4 has a newer edit of
it — useful for spotting which worktree to "base" a sync from.

`gwtp sideload` always copies files preserving the source's modification
time (like `cp -p`), instead of stamping the copy time — so the dates shown
above reflect when a file's *content* last changed, not when it was last
copied between worktrees. This only holds for files copied by `gwtp`; a
file's mtime from before you started using `gwtp sideload` (or copied by
some other tool) reflects whatever it was set to then.

### Prompt integration

```bash
gwtp prompt          # Small: "🌲 [N] branch"
gwtp prompt medium   # Box format with slug, index, path
gwtp prompt json     # JSON for oh-my-posh
```

## How worktrees are organized

New worktrees are created at `~/.worktrees/<branch-name>/<repo-name>`. Environment files (`.env*`) and sideload patterns are automatically copied into new worktrees.

## Hidden worktrees

Worktrees whose directory name starts with `_` or `emdash` are hidden from listings by default (use `--all` to show them). Branches starting with `_` are also hidden.

## License

MIT
