use crate::git::{git_common_dir, git_toplevel, is_in_git_repo};
use crate::worktree::sorted_worktrees;
use std::path::Path;

/// wtt: Get or set the repo slug (stored in git-common-dir/repo_slug, shared across all worktrees)
pub fn cmd_slug(slug_text: Option<String>) {
    if !is_in_git_repo() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
    let common_dir = git_common_dir().unwrap_or_else(|| {
        eprintln!("❌ Could not determine common git dir.");
        std::process::exit(1);
    });
    let slug_file = Path::new(&common_dir).join("repo_slug");

    match slug_text {
        None => {
            if slug_file.exists() {
                let content = std::fs::read_to_string(&slug_file).unwrap_or_default();
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    println!("{}", trimmed);
                    return;
                }
            }
            // Fallback: main worktree directory name
            let main_root = crate::git::git_main_root().unwrap_or_default();
            let name = Path::new(&main_root)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&main_root);
            println!("{}", name);
        }
        Some(text) => {
            let trimmed = text.trim().to_string();
            std::fs::write(&slug_file, format!("{}\n", trimmed)).unwrap_or_else(|e| {
                eprintln!("❌ Failed to save slug: {}", e);
                std::process::exit(1);
            });
            eprintln!("✅ Repo slug set to '{}' for all worktrees.", trimmed);
        }
    }
}

/// Get current worktree index (1-based), returns None if main or not in WT
pub fn get_worktree_index() -> Option<usize> {
    if !is_in_git_repo() {
        return None;
    }
    let wts = sorted_worktrees();
    if wts.len() <= 1 {
        return None;
    }
    let current = git_toplevel()?;
    let pos = wts.iter().position(|wt| wt.path == current)?;
    if pos == 0 {
        None // main worktree
    } else {
        Some(pos + 1)
    }
}

/// Get the repo slug (common dir / repo_slug file or fallback)
pub fn get_slug() -> String {
    if !is_in_git_repo() {
        return String::new();
    }
    let common_dir = match git_common_dir() {
        Some(d) => d,
        None => return String::new(),
    };
    let slug_file = Path::new(&common_dir).join("repo_slug");
    if slug_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&slug_file) {
            let s = content.trim().to_string();
            if !s.is_empty() {
                return s;
            }
        }
    }
    // Fallback: main worktree dir name
    let main_root = crate::git::git_main_root().unwrap_or_default();
    Path::new(&main_root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string()
}

/// Get path relative to repo root, prefixed with repo name
/// e.g. "myrepo/src/components"
pub fn get_relative_path() -> Option<String> {
    if !is_in_git_repo() {
        return None;
    }
    let repo_root = git_toplevel()?;
    let current = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .ok()?;
    let repo_name = Path::new(&repo_root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    if current == repo_root {
        Some(repo_name)
    } else {
        let rel = current.strip_prefix(&format!("{}/", repo_root)).unwrap_or(&current);
        Some(format!("{}/{}", repo_name, rel))
    }
}
