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
    #[serde(default)]
    pub sync_patterns: Vec<String>,
}

fn default_hidden_wt() -> Vec<String> {
    vec!["_".into(), "emdash".into()]
}

fn default_hidden_br() -> Vec<String> {
    vec!["_".into()]
}

impl Default for GwtpConfig {
    fn default() -> Self {
        Self {
            hidden_wt_prefixes: default_hidden_wt(),
            hidden_branch_prefixes: default_hidden_br(),
            sync_patterns: Vec::new(),
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
    fs::write(&path, content).map_err(|e| format!("write config: {}", e))?;
    sync_managed_block(common_git_dir, config)
}

pub fn sync_managed_block(common_git_dir: &str, config: &GwtpConfig) -> Result<(), String> {
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

    if !config.sync_patterns.is_empty() {
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push('\n');
        new_content.push_str(MANAGED_BEGIN);
        new_content.push('\n');
        for p in &config.sync_patterns {
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

pub fn cmd_config(args: &[String], common_git_dir: &str) {
    if args.is_empty() {
        eprintln!("Usage: gwtp config <add|rm|list|set-hidden-wt|set-hidden-br> [args...]");
        return;
    }
    let mut config = load_config(common_git_dir);
    match args[0].as_str() {
        "list" => {
            println!("hidden_wt_prefixes: {:?}", config.hidden_wt_prefixes);
            println!("hidden_branch_prefixes: {:?}", config.hidden_branch_prefixes);
            println!("sync_patterns: {:?}", config.sync_patterns);
        }
        "add" => {
            if args.len() < 2 {
                eprintln!("Usage: gwtp config add <pattern>");
                return;
            }
            let pattern = args[1..].join(" ");
            if config.sync_patterns.contains(&pattern) {
                eprintln!("Pattern already exists: {}", pattern);
                return;
            }
            config.sync_patterns.push(pattern.clone());
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Added pattern: {}", pattern),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        "rm" => {
            if args.len() < 2 {
                eprintln!("Usage: gwtp config rm <pattern>");
                return;
            }
            let pattern = args[1..].join(" ");
            let before = config.sync_patterns.len();
            config.sync_patterns.retain(|p| p != &pattern);
            if config.sync_patterns.len() == before {
                eprintln!("Pattern not found: {}", pattern);
                return;
            }
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Removed pattern: {}", pattern),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        "set-hidden-wt" => {
            config.hidden_wt_prefixes = args[1..].iter().map(|s| s.clone()).collect();
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Updated hidden WT prefixes: {:?}", config.hidden_wt_prefixes),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        "set-hidden-br" => {
            config.hidden_branch_prefixes = args[1..].iter().map(|s| s.clone()).collect();
            match save_config(common_git_dir, &config) {
                Ok(_) => eprintln!("✅ Updated hidden branch prefixes: {:?}", config.hidden_branch_prefixes),
                Err(e) => eprintln!("❌ Error: {}", e),
            }
        }
        _ => eprintln!("Unknown config command: {}", args[0]),
    }
}
