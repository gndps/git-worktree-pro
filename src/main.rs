mod config;
mod date;
mod diff;
mod git;
mod list;
mod navigate;
mod notes;
mod ops;
mod prompt;
mod sideload;
mod slug;
mod status;
mod worktree;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gwtp", version, about = "Git Worktree Pro — comprehensive worktree management")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List worktrees
    List {
        /// Show hidden worktrees
        #[arg(short, long)]
        all: bool,
        /// Show git status for each worktree
        #[arg(short, long)]
        status: bool,
    },
    /// List worktrees with detailed git status
    ListDetail {
        #[arg(short, long)]
        all: bool,
    },
    /// List worktrees with recent commit log
    ListLog {
        #[arg(short, long)]
        all: bool,
    },
    /// Tree view with commit history
    Tree {
        /// Number of commits to show per worktree
        #[arg(default_value = "10")]
        count: usize,
        #[arg(short, long)]
        all: bool,
    },
    /// Create a new worktree — prints `cd '<path>'` to stdout
    Add {
        /// Branch name to create or checkout
        branch: String,
        /// Create branch from this source branch
        #[arg(long)]
        from: Option<String>,
    },
    /// Create a worktree with a random suffix
    AddRandom,
    /// Manage sideloaded files: patterns, broadcasting, and inspection
    Sideload {
        #[command(subcommand)]
        command: SideloadCommands,
    },
    /// Navigate to a worktree (prints `cd '<path>'` for eval)
    Cd {
        target: String,
    },
    /// Open worktree in editor
    Open {
        target: String,
    },
    /// Pick and open worktree with fzf
    OpenPick {
        #[arg(short, long)]
        all: bool,
    },
    /// Copy worktree folder name to clipboard
    CopyName {
        index: usize,
    },
    /// Rename worktree directory
    Rename {
        /// Worktree to rename (index, branch, or name)
        target: String,
        /// New directory name
        new_name: String,
    },
    /// Remove a worktree
    Remove {
        target: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Diff committed changes between two worktrees
    Diff {
        parent_index: usize,
        child_index: usize,
    },
    /// Open two worktrees in editor
    DiffCode {
        parent_index: usize,
        child_index: usize,
    },
    /// Numstat diff between two worktrees
    DiffList {
        parent_index: usize,
        child_index: usize,
    },
    /// All (working dir) diff between two worktrees
    DiffAll {
        parent_index: usize,
        child_index: usize,
    },
    /// Get or set a note for the current worktree
    Note {
        /// Note text to set (omit to get current note)
        text: Vec<String>,
    },
    /// Get or set the repo slug
    Slug {
        /// Slug to set (omit to get current slug)
        name: Option<String>,
    },
    /// Show prompt info for shell / oh-my-posh integration
    Prompt {
        /// Output format: small (default), medium (banner), json
        #[arg(default_value = "small")]
        format: String,
        /// Prefix output with 🌲 tree emoji
        #[arg(long)]
        emoji: bool,
        /// Use toilet rendering; optionally specify font name (default: future)
        #[arg(long, value_name = "FONT", num_args = 0..=1, default_missing_value = "future")]
        toilet: Option<String>,
        /// Use figlet rendering; optionally specify font name (default: standard)
        #[arg(long, value_name = "FONT", num_args = 0..=1, default_missing_value = "")]
        figlet: Option<String>,
    },
    /// Show compact git status
    Status,
    /// Manage gwtp configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current config
    List,
    /// Set hidden worktree directory-name prefixes
    SetHiddenWt {
        prefixes: Vec<String>,
    },
    /// Set hidden branch-name prefixes
    SetHiddenBr {
        prefixes: Vec<String>,
    },
    /// Set the editor command used by `gwtp config edit` / `gwtp sideload edit`
    SetEditor {
        editor: String,
    },
    /// Open gwtp.json in the configured editor
    Edit,
    #[command(hide = true)]
    Add {
        pattern: Vec<String>,
    },
    #[command(hide = true)]
    Rm {
        pattern: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SideloadCommands {
    /// Sideload into all visible worktrees from main root
    Init {
        #[arg(short, long)]
        all: bool,
    },
    /// Broadcast sideload files from a source worktree to all others
    Base {
        /// Source: "latest", a worktree index, or omit for current
        #[arg(long)]
        from: Option<String>,
        /// Specific paths to broadcast instead of sideload-managed files
        paths: Vec<String>,
    },
    /// Copy sideload files from specified worktree into current
    CpFrom {
        /// Source worktree (index, branch, or name)
        target: String,
    },
    /// Copy sideload files from current worktree into specified
    CpTo {
        target: String,
    },
    /// Add a sideload pattern (gitignore syntax)
    Add {
        pattern: Vec<String>,
    },
    /// Remove a sideload pattern
    Rm {
        pattern: Vec<String>,
    },
    /// Show configured sideload patterns
    #[command(visible_alias = "patterns")]
    ListPatterns,
    /// Open the sideload_patterns file in the configured editor
    Edit,
    /// Tree of sideloaded files in the current worktree
    #[command(visible_alias = "l")]
    List,
    /// Global tree of sideloaded files across all worktrees
    #[command(visible_alias = "la")]
    ListAll,
}

fn main() {
    // Allow `gwtp sideload --list`/`-l`/`--list-all`/`-la` to also be spelled
    // as bare subcommand tokens (`list`/`l`/`list-all`/`la`), since clap
    // subcommand aliases can't start with a dash.
    let raw_args: Vec<String> = normalize_args(std::env::args().collect());
    let cli = Cli::parse_from(raw_args);

    // Resolve common git dir (needed for config operations)
    let common_git_dir = git::git_common_dir().unwrap_or_default();
    let cfg = if !common_git_dir.is_empty() {
        config::load_config(&common_git_dir)
    } else {
        config::GwtpConfig::default()
    };

    match cli.command {
        Commands::List { all, status } => {
            list::cmd_list(all, status, &cfg);
        }
        Commands::ListDetail { all } => {
            list::cmd_list_detail(all, &cfg);
        }
        Commands::ListLog { all } => {
            list::cmd_list_log(all, &cfg);
        }
        Commands::Tree { count, all } => {
            list::cmd_tree(count, all, &cfg);
        }
        Commands::Add { branch, from } => {
            require_git();
            ops::cmd_add(&branch, from.as_deref(), &common_git_dir);
        }
        Commands::AddRandom => {
            require_git();
            ops::cmd_add_random(&common_git_dir);
        }
        Commands::Sideload { command } => {
            require_git();
            match command {
                SideloadCommands::Init { all } => sideload::cmd_init(all, &cfg, &common_git_dir),
                SideloadCommands::Base { from, paths } => {
                    sideload::cmd_base(from.as_deref(), &paths, &cfg, &common_git_dir)
                }
                SideloadCommands::CpFrom { target } => sideload::cmd_cp_from(&target, &common_git_dir),
                SideloadCommands::CpTo { target } => sideload::cmd_cp_to(&target, &common_git_dir),
                SideloadCommands::Add { pattern } => {
                    sideload::cmd_add_pattern(&common_git_dir, &pattern.join(" "))
                }
                SideloadCommands::Rm { pattern } => {
                    sideload::cmd_rm_pattern(&common_git_dir, &pattern.join(" "))
                }
                SideloadCommands::ListPatterns => sideload::cmd_list_patterns(&common_git_dir),
                SideloadCommands::Edit => sideload::cmd_edit(&common_git_dir, &cfg),
                SideloadCommands::List => sideload::cmd_list(&common_git_dir),
                SideloadCommands::ListAll => sideload::cmd_list_all(&common_git_dir),
            }
        }
        Commands::Cd { target } => {
            require_git();
            navigate::cmd_cd(&target);
        }
        Commands::Open { target } => {
            require_git();
            navigate::cmd_open(&target, &cfg);
        }
        Commands::OpenPick { all } => {
            require_git();
            navigate::cmd_open_pick(all, &cfg);
        }
        Commands::CopyName { index } => {
            require_git();
            navigate::cmd_copy_name(index);
        }
        Commands::Rename { target, new_name } => {
            require_git();
            ops::cmd_rename(&target, &new_name);
        }
        Commands::Remove { target, force } => {
            require_git();
            ops::cmd_remove(&target, force);
        }
        Commands::Diff { parent_index, child_index } => {
            require_git();
            diff::cmd_diff(parent_index, child_index);
        }
        Commands::DiffCode { parent_index, child_index } => {
            require_git();
            diff::cmd_diff_code(parent_index, child_index, &cfg);
        }
        Commands::DiffList { parent_index, child_index } => {
            require_git();
            diff::cmd_diff_list(parent_index, child_index);
        }
        Commands::DiffAll { parent_index, child_index } => {
            require_git();
            diff::cmd_diff_all(parent_index, child_index);
        }
        Commands::Note { text } => {
            let note = if text.is_empty() {
                None
            } else {
                Some(text.join(" "))
            };
            notes::cmd_note(note);
        }
        Commands::Slug { name } => {
            slug::cmd_slug(name);
        }
        Commands::Prompt { format, emoji, toilet, figlet } => {
            let opts = prompt::PromptOpts { emoji, toilet, figlet };
            match format.as_str() {
                "medium" => prompt::cmd_prompt_medium(&opts),
                "json" => prompt::cmd_prompt_json(),
                _ => prompt::cmd_prompt_small(emoji),
            }
        }
        Commands::Status => {
            status::cmd_status();
        }
        Commands::Config { command } => {
            if common_git_dir.is_empty() {
                eprintln!("❌ Not in a git repository.");
                std::process::exit(1);
            }
            config::cmd_config(command, &common_git_dir);
        }
    }
}

/// Rewrites `gwtp sideload -l/--list/-la/--list-all` into the equivalent bare
/// subcommand token (`l`/`list`/`la`/`list-all`) so both spellings work.
fn normalize_args(args: Vec<String>) -> Vec<String> {
    if args.len() < 3 || args[1] != "sideload" {
        return args;
    }
    let mut out = args;
    let token = out[2].as_str();
    let replacement = match token {
        "-l" | "--list" => Some("list"),
        "-la" | "--list-all" => Some("list-all"),
        _ => None,
    };
    if let Some(r) = replacement {
        out[2] = r.to_string();
    }
    out
}

fn require_git() {
    if !git::is_in_git_repo() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
}
