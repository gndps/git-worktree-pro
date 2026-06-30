use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const MANAGED_BEGIN: &str = "# --- BEGIN GIT_WORKTREE_PRO MANAGED BLOCK ---";
const MANAGED_END: &str = "# --- END GIT_WORKTREE_PRO MANAGED BLOCK ---";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GwtpConfig {
    #[serde(default = "default_hidden_wt")]
    pub hidden_wt_prefixes: Vec<String>,
    #[serde(default = "default_hidden_br")]
    pub hidden_branch_prefixes: Vec<String>,
    #[serde(default = "default_editor")]
    pub editor: String,
}

fn default_hidden_wt() -> Vec<String> {
    vec!["_".into(), "emdash".into()]
}

fn default_hidden_br() -> Vec<String> {
    vec!["_".into()]
}

fn default_editor() -> String {
    "vim".to_string()
}

impl Default for GwtpConfig {
    fn default() -> Self {
        Self {
            hidden_wt_prefixes: default_hidden_wt(),
            hidden_branch_prefixes: default_hidden_br(),
            editor: default_editor(),
        }
    }
}

fn config_path(common_git_dir: &str) -> PathBuf {
    Path::new(common_git_dir).join("gwtp.json")
}

pub fn load_config(common_git_dir: &str) -> GwtpConfig {
    let path = config_path(common_git_dir);
    if let Ok(content) = fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        GwtpConfig::default()
    }
}

pub fn save_config(common_git_dir: &str, config: &GwtpConfig) -> Result<(), String> {
    let path = config_path(common_git_dir);
    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("serialize: {}", e))?;
    fs::write(&path, content).map_err(|e| format!("write config: {}", e))
}

/// Rewrites the MANAGED BLOCK in `.git/info/exclude` to match `patterns`.
pub fn sync_managed_block(common_git_dir: &str, patterns: &[String]) -> Result<(), String> {
    let exclude_path = Path::new(common_git_dir).join("info").join("exclude");

    if exclude_path.is_symlink() {
        eprintln!(
            "⚠️  Warning: .git/info/exclude is a symlink. Managed block written to symlink target."
        );
    }

    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir info: {}", e))?;
    }

    let existing = fs::read_to_string(&exclude_path).unwrap_or_default();
    let mut new_content = remove_managed_block(&existing);

    if !patterns.is_empty() {
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push('\n');
        new_content.push_str(MANAGED_BEGIN);
        new_content.push('\n');
        for p in patterns {
            new_content.push_str(p);
            new_content.push('\n');
        }
        new_content.push_str(MANAGED_END);
        new_content.push('\n');
    }

    fs::write(&exclude_path, new_content).map_err(|e| format!("write exclude: {}", e))
}

fn remove_managed_block(content: &str) -> String {
    let mut result = String::new();
    let mut in_block = false;
    for line in content.lines() {
        if line == MANAGED_BEGIN {
            in_block = true;
            continue;
        }
        if line == MANAGED_END {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    // Trim trailing blank lines introduced by removing the block
    result.trim_end().to_string() + "\n"
}

pub fn cmd_config(command: crate::ConfigCommands, common_git_dir: &str) {
    use crate::ConfigCommands;

    let mut config = load_config(common_git_dir);
    match command {
        ConfigCommands::List => {
            println!("editor:                {}", config.editor);
            println!("hidden_wt_prefixes:    {:?}", config.hidden_wt_prefixes);
            println!("hidden_branch_prefixes:{:?}", config.hidden_branch_prefixes);
        }
        ConfigCommands::SetHiddenWt { prefixes } => {
            config.hidden_wt_prefixes = prefixes;
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Updated hidden WT prefixes: {:?}", config.hidden_wt_prefixes),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        ConfigCommands::SetHiddenBr { prefixes } => {
            config.hidden_branch_prefixes = prefixes;
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Updated hidden branch prefixes: {:?}", config.hidden_branch_prefixes),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        ConfigCommands::SetEditor { editor } => {
            config.editor = editor;
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Editor set to: {}", config.editor),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        ConfigCommands::Edit => {
            let path = config_path(common_git_dir);
            if !path.exists() {
                // Create default config file so there is something to edit
                if let Err(e) = save_config(common_git_dir, &config) {
                    eprintln!("❌ Could not create config file: {}", e);
                    return;
                }
            }
            std::process::Command::new(&config.editor)
                .arg(path.to_string_lossy().as_ref())
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("❌ Failed to open editor '{}': {}", config.editor, e);
                    std::process::exit(1);
                });
        }
        ConfigCommands::Add { .. } => {
            eprintln!("⚠️  Sideload patterns moved: use `gwtp sideload add <pattern>` instead.");
        }
        ConfigCommands::Rm { .. } => {
            eprintln!("⚠️  Sideload patterns moved: use `gwtp sideload rm <pattern>` instead.");
        }
    }
}
