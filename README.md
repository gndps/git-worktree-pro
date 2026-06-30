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
controlled by a standalone, gitignore-style pattern file at
`<git-common-dir>/sideload_patterns` — one pattern per line, `#` for
comments.

| Command | Description |
|---------|-------------|
| `gwtp sideload init [--all]` | Sideload into all visible worktrees from main root |
| `gwtp sideload base [--from <spec>] [paths...]` | Broadcast sideload files from a source worktree to all others |
| `gwtp sideload cp-from <wt>` | Copy sideload files from worktree into current |
| `gwtp sideload cp-to <wt>` | Copy sideload files from current into worktree |
| `gwtp sideload add <pattern>` | Add a pattern |
| `gwtp sideload rm <pattern>` | Remove a pattern |
| `gwtp sideload list-patterns` (alias `patterns`) | Show configured patterns |
| `gwtp sideload edit` | Open `sideload_patterns` in your configured editor |
| `gwtp sideload list` (`-l` / `l` / `--list`) | Tree of sideloaded files in the current worktree |
| `gwtp sideload list-all` (`-la` / `la` / `--list-all`) | Global tree of sideloaded files across every worktree — each unique path shows a 6-char content hash and the worktree indices holding that version, so you can spot divergence before basing/copying files between worktrees |

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
