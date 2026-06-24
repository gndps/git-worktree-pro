use crate::config::load_config;
use crate::git::{git_common_dir, git_in, git_main_root, git_toplevel, git_current_branch};
use crate::sync::{copy_paths_to, sync_worktree_files};
use crate::worktree::{get_wt_config_mtime, is_worktree_hidden, sorted_worktrees, get_worktree_path, Worktree};
use crate::config::GwtpConfig;
use std::path::{Path, PathBuf};
use std::process::Command;

/// wta: Create a new worktree at ~/.worktrees/<branch>/<repo-name>
/// Prints `cd '<path>'` to stdout on success (for eval by shell wrapper).
/// All status output goes to stderr.
pub fn cmd_add(
    branch: &str,
    from: Option<&str>,
    common_git_dir: &str,
) {
    let current_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });
    let main_root = git_main_root().unwrap_or(current_root.clone());

    let git_dir_name = Path::new(&main_root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let target = format!("{home}/.worktrees/{branch}/{git_dir_name}");

    eprintln!("📂 Preparing: {}", target);
    if let Some(parent) = Path::new(&target).parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("❌ Failed to create directory: {}", e);
            std::process::exit(1);
        }
    }

    // Check if branch already exists locally or on origin
    let branch_exists_local = git_in(None, &["rev-parse", "--verify", branch]).is_ok();
    let branch_exists_remote =
        git_in(None, &["rev-parse", "--verify", &format!("origin/{}", branch)]).is_ok();

    if branch_exists_local || branch_exists_remote {
        eprintln!("🌿 Branch '{}' found. Checking out...", branch);
        let status = Command::new("git")
            .args(["worktree", "add", &target, branch])
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            std::process::exit(1);
        }
    } else {
        // Create new branch
        match from {
            Some(src) => {
                let start = if git_in(None, &["rev-parse", "--verify", src]).is_ok() {
                    src.to_string()
                } else if git_in(None, &["rev-parse", "--verify", &format!("origin/{}", src)]).is_ok() {
                    format!("origin/{}", src)
                } else {
                    eprintln!("❌ Source branch '{}' not found.", src);
                    std::process::exit(1);
                };
                eprintln!("🌱 Creating '{}' from '{}'...", branch, start);
                let status = Command::new("git")
                    .args(["worktree", "add", "-b", branch, &target, &start])
                    .status();
                if !status.map(|s| s.success()).unwrap_or(false) {
                    std::process::exit(1);
                }
            }
            None => {
                eprintln!("🌱 Creating new branch '{}' from current HEAD...", branch);
                let status = Command::new("git")
                    .args(["worktree", "add", "-b", branch, &target])
                    .status();
                if !status.map(|s| s.success()).unwrap_or(false) {
                    std::process::exit(1);
                }
            }
        }
    }

    sync_worktree_files(&current_root, &target, false, common_git_dir);
    eprintln!("🎉 Worktree ready at: {}", target);

    // Print cd command to stdout for eval
    println!("cd '{}'", target);
}

/// wtar: Add worktree with random suffix on current branch name
pub fn cmd_add_random(common_git_dir: &str) {
    let current_branch = git_current_branch().unwrap_or_else(|| "branch".to_string());
    let rand_suffix = rand_hex(4);
    let new_branch = format!("{}-{}", current_branch, rand_suffix);
    eprintln!("🎲 Base branch: {}", current_branch);
    eprintln!("🔀 New branch name: {}", new_branch);
    cmd_add(&new_branch, None, common_git_dir);
}

fn rand_hex(bytes: usize) -> String {
    if let Ok(out) = Command::new("openssl").args(["rand", "-hex", &bytes.to_string()]).output() {
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    } else {
        format!("{:04x}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0) % 65536)
    }
}

/// wtin: Initialize (sync) all visible worktrees from main root
pub fn cmd_init(show_all: bool, config: &GwtpConfig, common_git_dir: &str) {
    let main_root = git_main_root().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    if show_all {
        eprintln!("🔄 Initializing ALL worktrees (including hidden) from main root: {}", main_root);
    } else {
        eprintln!("🔄 Initializing visible worktrees from main root: {}", main_root);
    }

    let wts = sorted_worktrees();
    let mut count = 0usize;

    for wt in &wts {
        if wt.path == main_root {
            continue;
        }
        if !show_all && is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes) {
            continue;
        }
        eprintln!("--------------------------------------------------------");
        sync_worktree_files(&main_root, &wt.path, false, common_git_dir);
        count += 1;
    }

    eprintln!("--------------------------------------------------------");
    if count == 0 {
        eprintln!("ℹ️  No additional worktrees found to initialize.");
    } else {
        eprintln!("🎉 Successfully initialized {} worktree(s)!", count);
    }
}

/// wtbase: Broadcast config files from source worktree to all others
pub fn cmd_base(
    from_spec: Option<&str>,
    extra_paths: &[String],
    config: &GwtpConfig,
    common_git_dir: &str,
) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }

    let source_root: String = match from_spec {
        Some("latest") => {
            eprintln!("🔍 Finding worktree with most recently changed config files...");
            let (best, _) = wts.iter()
                .map(|wt| (wt.path.as_str(), get_wt_config_mtime(&wt.path)))
                .max_by_key(|&(_, m)| m)
                .unwrap_or((&wts[0].path, 0));
            let p = best.to_string();
            eprintln!("📌 Latest config source: {}", p);
            p
        }
        Some(spec) => {
            let p = get_worktree_path(spec, &wts).unwrap_or_else(|| {
                eprintln!("❌ No worktree found for '{}'.", spec);
                std::process::exit(1);
            });
            eprintln!("📌 Source worktree: {}", p);
            p
        }
        None => git_toplevel().unwrap_or_else(|| {
            eprintln!("❌ Not in a git repository.");
            std::process::exit(1);
        }),
    };

    let mut count = 0usize;

    if extra_paths.is_empty() {
        // Broadcast standard config files
        eprintln!("🔄 Broadcasting worktree config files from: {}", source_root);
        for wt in &wts {
            if wt.path == source_root {
                continue;
            }
            eprintln!("--------------------------------------------------------");
            sync_worktree_files(&source_root, &wt.path, true, common_git_dir);
            count += 1;
        }
    } else {
        // Copy specific paths
        eprintln!("🔄 Broadcasting specified paths from: {}", source_root);
        let resolved: Vec<PathBuf> = extra_paths.iter().filter_map(|p| {
            let abs = if p.starts_with('/') {
                PathBuf::from(p)
            } else {
                std::env::current_dir().unwrap_or_default().join(p)
            };
            if !abs.exists() {
                eprintln!("⚠️  Warning: Path not found, skipping: {}", p);
                return None;
            }
            if !abs.starts_with(&source_root) {
                eprintln!("⚠️  Warning: Path outside source worktree, skipping: {}", p);
                return None;
            }
            Some(abs)
        }).collect();

        if resolved.is_empty() {
            eprintln!("❌ No valid paths to copy.");
            std::process::exit(1);
        }

        for wt in &wts {
            if wt.path == source_root {
                continue;
            }
            eprintln!("--------------------------------------------------------");
            eprintln!("📂 Copying to: {}", wt.path);
            copy_paths_to(&source_root, &wt.path, &resolved);
            count += 1;
        }
    }

    eprintln!("--------------------------------------------------------");
    if count == 0 {
        eprintln!("ℹ️  No other worktrees to sync to.");
    } else {
        eprintln!("🎉 Successfully synced to {} worktree(s)!", count);
    }
}

/// wtcpfrom: Copy config files from specified WT into current WT
pub fn cmd_cp_from(target: &str, common_git_dir: &str) {
    let wts = sorted_worktrees();
    let source_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ No worktree found for '{}'.", target);
        std::process::exit(1);
    });
    let current_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });
    if source_path == current_root {
        eprintln!("❌ Source and target are the same worktree.");
        std::process::exit(1);
    }
    eprintln!("📥 Copying config files from: {}", source_path);
    eprintln!("   To (current worktree):     {}", current_root);
    sync_worktree_files(&source_path, &current_root, false, common_git_dir);
    eprintln!("🎉 Done.");
}

/// wtcpto: Copy config files from current WT to specified WT
pub fn cmd_cp_to(target: &str, common_git_dir: &str) {
    let wts = sorted_worktrees();
    let target_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ No worktree found for '{}'.", target);
        std::process::exit(1);
    });
    let current_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });
    if target_path == current_root {
        eprintln!("❌ Source and target are the same worktree.");
        std::process::exit(1);
    }
    eprintln!("📤 Copying config files from (current worktree): {}", current_root);
    eprintln!("   To: {}", target_path);
    sync_worktree_files(&current_root, &target_path, false, common_git_dir);
    eprintln!("🎉 Done.");
}

/// wrm: Remove a worktree
pub fn cmd_remove(target: &str, force: bool) {
    let wts = sorted_worktrees();
    let target_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ Worktree '{}' not found.", target);
        std::process::exit(1);
    });
    let current_root = git_toplevel().unwrap_or_default();
    if target_path == current_root {
        let main_root = git_main_root().unwrap_or_default();
        if target_path == main_root {
            eprintln!("❌ Cannot remove the main worktree.");
            std::process::exit(1);
        }
    }

    if force {
        eprintln!("🗑️  Force removing worktree: {}", target_path);
        Command::new("git")
            .args(["worktree", "remove", "--force", &target_path])
            .status()
            .ok();
    } else {
        eprintln!("🗑️  Removing worktree: {}", target_path);
        let status = Command::new("git")
            .args(["worktree", "remove", &target_path])
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            std::process::exit(1);
        }
    }
}

/// wrn / wren: Rename worktree directory
pub fn cmd_rename(target: &str, new_name: &str) {
    let wts = sorted_worktrees();
    let target_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ Worktree '{}' not found.", target);
        std::process::exit(1);
    });
    let main_root = git_main_root().unwrap_or_default();
    if target_path == main_root {
        eprintln!("❌ Cannot rename the main repository worktree.");
        std::process::exit(1);
    }
    let parent = Path::new(&target_path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let new_path = format!("{}/{}", parent, new_name);
    if Path::new(&new_path).exists() {
        eprintln!("❌ Directory '{}' already exists.", new_name);
        std::process::exit(1);
    }
    if let Some(p) = Path::new(&new_path).parent() {
        let _ = std::fs::create_dir_all(p);
    }
    eprintln!("🔄 Renaming worktree:");
    eprintln!("   From: {}", target_path);
    eprintln!("   To:   {}", new_path);
    let status = Command::new("git")
        .args(["worktree", "move", &target_path, &new_path])
        .status();
    if status.map(|s| s.success()).unwrap_or(false) {
        eprintln!("✅ Worktree renamed successfully (branch name unchanged).");
    } else {
        eprintln!("❌ Failed to rename worktree.");
        std::process::exit(1);
    }
}
