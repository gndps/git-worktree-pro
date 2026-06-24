# git-worktree-pro

A comprehensive git worktree management toolkit, available as a single binary (`gwtp`).

## Install

```bash
brew install gndps/tap/git-worktree-pro
```

## Setup

Add to your `.bashrc` or `.zshrc`:

```bash
# Source the aliases file installed by gwtp
source "$(gwtp --aliases-path 2>/dev/null)" 2>/dev/null
```

Or manually source the bundled aliases:

```bash
# Core aliases
alias wls='gwtp list'
alias wlst='gwtp list --status'
alias wlsd='gwtp list-detail'
alias wlsl='gwtp list-log'
alias wll='gwtp tree'
wta() { eval "$(gwtp add "$@")"; }
wtar() { eval "$(gwtp add-random)"; }
alias wtin='gwtp init'
wcd() { eval "$(gwtp cd "$@")"; }
alias wwi='gwtp open'
alias wwif='gwtp open-pick'
alias wcp='gwtp copy-name'
alias wrn='gwtp rename'
alias wrm='gwtp remove'
alias wdiff='gwtp diff'
alias wtn='gwtp note'
alias wtt='gwtp slug'
alias gsd='gwtp status'
```

## Commands

| Alias | Command | Description |
|-------|---------|-------------|
| `wls` | `gwtp list` | List worktrees |
| `wlst` | `gwtp list --status` | List with git status |
| `wlsd` | `gwtp list-detail` | List with full status |
| `wlsl` | `gwtp list-log` | List with recent commits |
| `wll` | `gwtp tree` | Tree view with commit history |
| `wta` | `gwtp add <branch>` | Create worktree at `~/.worktrees/<branch>/<repo>` |
| `wtar` | `gwtp add-random` | Create worktree with random suffix |
| `wtin` | `gwtp init` | Sync all worktrees from main |
| `wtbase` | `gwtp base` | Broadcast config files to all worktrees |
| `wtcpfrom` | `gwtp cp-from <wt>` | Copy config from WT to current |
| `wtcpto` | `gwtp cp-to <wt>` | Copy config from current to WT |
| `wcd` | `gwtp cd <wt>` | Navigate to worktree |
| `wwi` | `gwtp open <wt>` | Open worktree in Windsurf |
| `wwif` | `gwtp open-pick` | Pick worktree with fzf |
| `wcp` | `gwtp copy-name <N>` | Copy folder name to clipboard |
| `wrn` | `gwtp rename <wt> <name>` | Rename worktree directory |
| `wrm` | `gwtp remove <wt>` | Remove worktree |
| `wdiff` | `gwtp diff <P> <C>` | Diff between worktrees |
| `wdiffc` | `gwtp diff-code <P> <C>` | Diff in Windsurf |
| `wdiffl` | `gwtp diff-list <P> <C>` | Numstat diff |
| `wdiffa` | `gwtp diff-all <P> <C>` | Working dir diff |
| `wtn` | `gwtp note [text]` | Get/set worktree note |
| `wtt` | `gwtp slug [name]` | Get/set repo slug |
| `gsd` | `gwtp status` | Compact git status |

### Config management

```bash
gwtp config list                   # Show current config
gwtp config add <pattern>          # Add sync pattern
gwtp config rm <pattern>           # Remove sync pattern
gwtp config set-hidden-wt _ emdash # Set hidden WT prefixes
gwtp config set-hidden-br _        # Set hidden branch prefixes
```

### Prompt integration

```bash
gwtp prompt          # Small: "🌲 [N] branch"
gwtp prompt medium   # Box format with slug, index, path
gwtp prompt json     # JSON for oh-my-posh
```

## How worktrees are organized

New worktrees are created at `~/.worktrees/<branch-name>/<repo-name>`. Environment files (`.env*`) and patterns from `gwtp config` are automatically synced to new worktrees.

## Hidden worktrees

Worktrees whose directory name starts with `_` or `emdash` are hidden from listings by default (use `--all` to show them). Branches starting with `_` are also hidden.

## License

MIT
