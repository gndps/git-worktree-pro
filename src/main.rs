mod config;
mod diff;
mod git;
mod list;
mod navigate;
mod notes;
mod ops;
mod prompt;
mod slug;
mod status;
mod sync;
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
    /// List worktrees [wls / wlst]
    List {
        /// Show hidden worktrees
        #[arg(short, long)]
        all: bool,
        /// Show git status for each worktree
        #[arg(short, long)]
        status: bool,
    },
    /// List worktrees with detailed git status [wlsd]
    ListDetail {
        #[arg(short, long)]
        all: bool,
    },
    /// List worktrees with recent commit log [wlsl]
    ListLog {
        #[arg(short, long)]
        all: bool,
    },
    /// Tree view with commit history [wll]
    Tree {
        /// Number of commits to show per worktree
        #[arg(default_value = "10")]
        count: usize,
        #[arg(short, long)]
        all: bool,
    },
    /// Create a new worktree [wta]  — prints `cd '<path>'` to stdout
    Add {
        /// Branch name to create or checkout
        branch: String,
        /// Create branch from this source branch
        #[arg(long)]
        from: Option<String>,
    },
    /// Create a worktree with a random suffix [wtar]
    AddRandom,
    /// Initialize (sync) all worktrees from main [wtin]
    Init {
        #[arg(short, long)]
        all: bool,
    },
    /// Broadcast config files to all worktrees [wtbase]
    Base {
        /// Source: "latest", a worktree index, or omit for current
        #[arg(long)]
        from: Option<String>,
        /// Specific paths to broadcast instead of config files
        paths: Vec<String>,
    },
    /// Copy config files from specified worktree to current [wtcpfrom]
    CpFrom {
        /// Target worktree (index, branch, or name)
        target: String,
    },
    /// Copy config files from current worktree to specified [wtcpto]
    CpTo {
        target: String,
    },
    /// Navigate to a worktree (prints `cd '<path>'` for eval) [wcd]
    Cd {
        target: String,
    },
    /// Open worktree in Windsurf [wwi]
    Open {
        target: String,
    },
    /// Pick and open worktree with fzf [wwif]
    OpenPick {
        #[arg(short, long)]
        all: bool,
    },
    /// Copy worktree folder name to clipboard [wcp]
    CopyName {
        index: usize,
    },
    /// Rename worktree directory [wrn / wren]
    Rename {
        /// Worktree to rename (index, branch, or name)
        target: String,
        /// New directory name
        new_name: String,
    },
    /// Remove a worktree [wrm]
    Remove {
        target: String,
        #[arg(short, long)]
        force: bool,
    },
    /// Diff committed changes between two worktrees [wdiff]
    Diff {
        parent_index: usize,
        child_index: usize,
    },
    /// Open diff between two worktrees in Windsurf [wdiffc]
    DiffCode {
        parent_index: usize,
        child_index: usize,
    },
    /// Numstat diff between two worktrees [wdiffl]
    DiffList {
        parent_index: usize,
        child_index: usize,
    },
    /// All (working dir) diff between two worktrees [wdiffa]
    DiffAll {
        parent_index: usize,
        child_index: usize,
    },
    /// Get or set a note for the current worktree [wtn]
    Note {
        /// Note text to set (omit to get current note)
        text: Vec<String>,
    },
    /// Get or set the repo slug [wtt]
    Slug {
        /// Slug to set (omit to get current slug)
        name: Option<String>,
    },
    /// Show prompt info
    Prompt {
        /// Format: small (default), medium, or json
        #[arg(default_value = "small")]
        format: String,
    },
    /// Show compact git status [gsd]
    Status,
    /// Manage gwtp configuration
    Config {
        /// Subcommand: list, add <pattern>, rm <pattern>, set-hidden-wt, set-hidden-br
        args: Vec<String>,
    },
}

fn main() {
    let cli = Cli::parse();

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
        Commands::Init { all } => {
            require_git();
            ops::cmd_init(all, &cfg, &common_git_dir);
        }
        Commands::Base { from, paths } => {
            require_git();
            ops::cmd_base(from.as_deref(), &paths, &cfg, &common_git_dir);
        }
        Commands::CpFrom { target } => {
            require_git();
            ops::cmd_cp_from(&target, &common_git_dir);
        }
        Commands::CpTo { target } => {
            require_git();
            ops::cmd_cp_to(&target, &common_git_dir);
        }
        Commands::Cd { target } => {
            require_git();
            navigate::cmd_cd(&target);
        }
        Commands::Open { target } => {
            require_git();
            navigate::cmd_open(&target);
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
            diff::cmd_diff_code(parent_index, child_index);
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
        Commands::Prompt { format } => match format.as_str() {
            "medium" => prompt::cmd_prompt_medium(),
            "json" => prompt::cmd_prompt_json(),
            _ => prompt::cmd_prompt_small(),
        },
        Commands::Status => {
            status::cmd_status();
        }
        Commands::Config { args } => {
            if common_git_dir.is_empty() {
                eprintln!("❌ Not in a git repository.");
                std::process::exit(1);
            }
            config::cmd_config(&args, &common_git_dir);
        }
    }
}

fn require_git() {
    if !git::is_in_git_repo() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
}
