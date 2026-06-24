use crate::config::GwtpConfig;
use crate::worktree::{get_worktree_path, sorted_worktrees};
use std::process::Command;

fn get_indexed_pair(p_idx: usize, c_idx: usize) -> (String, String, String, String) {
    let wts = sorted_worktrees();
    if p_idx < 1 || p_idx > wts.len() || c_idx < 1 || c_idx > wts.len() {
        eprintln!("❌ Invalid worktree index.");
        std::process::exit(1);
    }
    let p = &wts[p_idx - 1];
    let c = &wts[c_idx - 1];
    (p.path.clone(), p.branch.clone(), c.path.clone(), c.branch.clone())
}

/// wdiff: Show committed diff between two worktrees
pub fn cmd_diff(p_idx: usize, c_idx: usize) {
    let (p_path, p_branch, c_path, c_branch) = get_indexed_pair(p_idx, c_idx);
    eprintln!(
        "Diffing committed changes: \x1b[33m{}\x1b[0m ... \x1b[1;36m{}\x1b[0m",
        p_branch, c_branch
    );
    Command::new("git")
        .current_dir(&c_path)
        .args(["diff", &format!("{}...{}", p_branch, c_branch)])
        .status()
        .ok();
}

/// wdiffc: Open both worktrees side by side in the configured editor
pub fn cmd_diff_code(p_idx: usize, c_idx: usize, config: &GwtpConfig) {
    let (p_path, _, c_path, _) = get_indexed_pair(p_idx, c_idx);
    Command::new(&config.editor)
        .args([&p_path, &c_path])
        .status()
        .ok();
}

/// wdiffl: Show numstat diff between two worktrees
pub fn cmd_diff_list(p_idx: usize, c_idx: usize) {
    let (p_path, p_branch, c_path, c_branch) = get_indexed_pair(p_idx, c_idx);
    eprintln!(
        "Changes in \x1b[1;36m{}\x1b[0m compared to \x1b[33m{}\x1b[0m:",
        c_branch, p_branch
    );
    eprintln!("--------------------------------------------------------");
    let out = Command::new("git")
        .current_dir(&c_path)
        .args(["diff", "--numstat", &format!("{}...{}", p_branch, c_branch)])
        .output();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).lines() {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            if parts.len() == 3 {
                println!("+{:<6} -{:<6} {}", parts[0], parts[1], parts[2]);
            }
        }
    }
}

/// wdiffa: Show ALL (working dir) changes between two worktrees
pub fn cmd_diff_all(p_idx: usize, c_idx: usize) {
    let (p_path, p_branch, c_path, c_branch) = get_indexed_pair(p_idx, c_idx);
    eprintln!(
        "Diffing ALL (WIP) changes: \x1b[33m{}\x1b[0m -> \x1b[1;36m{} (Working Dir)\x1b[0m",
        p_branch, c_branch
    );
    Command::new("git")
        .current_dir(&c_path)
        .args(["diff", &p_branch])
        .status()
        .ok();
}
