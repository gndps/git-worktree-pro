use crate::config::GwtpConfig;
use crate::worktree::{get_wt_config_date, get_wt_config_mtime, get_wt_note, get_wt_status, is_worktree_hidden, sorted_worktrees, Worktree};
use std::process::Command;

pub const YELLOW: &str = "\x1b[33m";
pub const CYAN_BOLD: &str = "\x1b[1;36m";
pub const DIM: &str = "\x1b[2m";
pub const SKY_BLUE: &str = "\x1b[38;5;117m";
pub const GRAY: &str = "\x1b[38;5;245m";
pub const RESET: &str = "\x1b[0m";
pub const GREEN_BOLD: &str = "\x1b[1;32m";
pub const MAGENTA: &str = "\x1b[35m";
pub const WHITE: &str = "\x1b[0;37m";
pub const RED: &str = "\x1b[31m";

pub fn cmd_list(show_all: bool, show_status: bool, config: &GwtpConfig) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
    let current_root = crate::git::git_toplevel().unwrap_or_default();
    let visible: Vec<(usize, &Worktree)> = wts
        .iter()
        .enumerate()
        .filter(|(_, wt)| {
            show_all
                || !is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes)
        })
        .map(|(i, wt)| (i + 1, wt))
        .collect();

    let show_dates = {
        let mtimes: Vec<u64> = visible
            .iter()
            .map(|(_, wt)| get_wt_config_mtime(&wt.path))
            .filter(|&m| m > 0)
            .collect();
        mtimes.len() > 1 && mtimes.windows(2).any(|w| w[0] != w[1])
    };

    for (num, wt) in &visible {
        let marker = if wt.path == current_root { "*" } else { " " };
        let bd = wt.display_branch();
        let note = get_wt_note(&wt.path).unwrap_or_default();

        if show_status {
            let status = get_wt_status(&wt.path);
            if !status.is_empty() {
                println!(
                    "{}[{}{}]{} {}{}{}  {}{}{}",
                    YELLOW, marker, num, RESET, CYAN_BOLD, bd, RESET, GRAY, status, RESET
                );
            } else {
                println!(
                    "{}[{}{}]{} {}{}{}",
                    YELLOW, marker, num, RESET, CYAN_BOLD, bd, RESET
                );
            }
        } else {
            println!(
                "{}[{}{}]{} {}{}{}",
                YELLOW, marker, num, RESET, CYAN_BOLD, bd, RESET
            );
        }

        if !note.is_empty() {
            println!("    {}☁️  {}{}", SKY_BLUE, note, RESET);
        }
        println!("    {}{}{}", DIM, wt.path, RESET);

        if show_dates {
            if let Some(date) = get_wt_config_date(&wt.path) {
                println!("    {}📅 {}{}", GRAY, date, RESET);
            }
        }
    }
}

pub fn cmd_list_detail(show_all: bool, config: &GwtpConfig) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
    let visible: Vec<(usize, &Worktree)> = wts
        .iter()
        .enumerate()
        .filter(|(_, wt)| {
            show_all
                || !is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes)
        })
        .map(|(i, wt)| (i + 1, wt))
        .collect();

    for (num, wt) in &visible {
        let bd = wt.display_branch();
        let note = get_wt_note(&wt.path).unwrap_or_default();
        println!("{}[{}]{} {}{}{}", YELLOW, num, RESET, CYAN_BOLD, bd, RESET);
        if !note.is_empty() {
            println!("    {}☁️  {}{}", SKY_BLUE, note, RESET);
        }
        println!("    {}{}{}", DIM, wt.path, RESET);

        let status_out = Command::new("git")
            .current_dir(&wt.path)
            .args(["-c", "color.status=always", "status", "-sb"])
            .output();
        match status_out {
            Ok(o) => {
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    println!("    {}", line);
                }
            }
            Err(_) => println!("    {}! Directory missing{}", RED, RESET),
        }
    }
}

pub fn cmd_list_log(show_all: bool, config: &GwtpConfig) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
    let visible: Vec<(usize, &Worktree)> = wts
        .iter()
        .enumerate()
        .filter(|(_, wt)| {
            show_all
                || !is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes)
        })
        .map(|(i, wt)| (i + 1, wt))
        .collect();

    for (num, wt) in &visible {
        let bd = wt.display_branch();
        let note = get_wt_note(&wt.path).unwrap_or_default();
        println!("{}[{}]{} {}{}{}", YELLOW, num, RESET, CYAN_BOLD, bd, RESET);
        if !note.is_empty() {
            println!("    {}☁️  {}{}", SKY_BLUE, note, RESET);
        }
        println!("    {}{}{}", DIM, wt.path, RESET);

        let log_out = Command::new("git")
            .current_dir(&wt.path)
            .args([
                "log", "--color=always",
                "--format=%C(yellow)%h %C(green)%ad %C(blue)%an %C(cyan)%s",
                "-n", "5",
            ])
            .output();
        if let Ok(o) = log_out {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                println!("    {}", line);
            }
        }
    }
}

// ── Tree view ────────────────────────────────────────────────────────────────

struct TreeNode {
    wt_idx: usize,   // index into wts array
    id: usize,
    parent_idx: Option<usize>,  // index into tree_nodes
    base_sha: Option<String>,
    note: String,
    children: Vec<usize>,  // indices into tree_nodes
}

fn shorten_path(full: &str, root: &str) -> String {
    if full == root {
        return ".".to_string();
    }
    let root_parent = std::path::Path::new(root)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    if !root_parent.is_empty() && full.starts_with(&root_parent) {
        full[root_parent.len()..].trim_start_matches('/').to_string()
    } else {
        full.to_string()
    }
}

fn get_logs(path: &str, max: usize, base_sha: Option<&str>) -> Vec<String> {
    let n = max.to_string();
    let base_arg;
    let mut args: Vec<&str> = vec![
        "log", "--date=short", "--abbrev-commit", "--color=always",
        "--format=%C(yellow)%h %C(green)%ad %C(reset)%s",
        "-n", &n,
    ];
    if let Some(base) = base_sha {
        base_arg = format!("^{}", base);
        args.push("HEAD");
        args.push(&base_arg);
    }
    Command::new("git")
        .current_dir(path)
        .args(&args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(String::from).collect())
        .unwrap_or_default()
}

fn get_base_log_line(path: &str, sha: &str) -> Option<String> {
    Command::new("git")
        .current_dir(path)
        .args(["show", "-s", "--format=%C(yellow)%h %C(green)%ad %C(reset)%s", "--date=short", sha])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
}

fn print_tree_node(
    idx: usize,
    nodes: &[TreeNode],
    wts: &[Worktree],
    logs: &[Vec<String>],
    root_path: &str,
    prefix: &str,
    is_last: bool,
) {
    let node = &nodes[idx];
    let wt = &wts[node.wt_idx];
    let rel_path = shorten_path(&wt.path, root_path);
    let is_root = node.parent_idx.is_none();

    let mut label = format!(
        "{}[{}]{} {}{}{}  {}({}){} ",
        YELLOW, node.id, RESET,
        CYAN_BOLD, wt.display_branch(), RESET,
        DIM, rel_path, RESET
    );
    if is_root {
        label.push_str(&format!("{}(Main){}", DIM, RESET));
    }
    if !node.note.is_empty() {
        label.push_str(&format!("  {}☁️  {}{}", SKY_BLUE, node.note, RESET));
    }

    let child_prefix: String;
    if is_root {
        println!("{}", label);
        child_prefix = String::new();
    } else {
        let connector = if is_last { "└── " } else { "├── " };
        println!("{}{}{}{}", prefix, WHITE, connector, RESET);
        println!("{}  {}", prefix, label);
        child_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
    }

    let node_logs = &logs[idx];
    for log_line in node_logs {
        println!("{}{}├── {}{}", child_prefix, WHITE, RESET, log_line);
    }

    let has_children = !node.children.is_empty();
    if let Some(ref base) = node.base_sha {
        if let Some(base_line) = get_base_log_line(&wt.path, base) {
            let connector = if has_children { "├──" } else { "└──" };
            println!(
                "{}{}{}{}↓{} {}  {}(branch point){}",
                child_prefix, WHITE, connector, RESET, RESET,
                base_line, DIM, RESET
            );
        }
    }

    let count = node.children.len();
    for (ci, &child_idx) in node.children.iter().enumerate() {
        print_tree_node(child_idx, nodes, wts, logs, root_path, &child_prefix, ci == count - 1);
    }
}

pub fn cmd_tree(max_commits: usize, show_all: bool, config: &GwtpConfig) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }

    let wt_indices: Vec<usize> = wts
        .iter()
        .enumerate()
        .filter(|(_, wt)| {
            show_all
                || wt.is_main
                || !is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes)
        })
        .map(|(i, _)| i)
        .collect();

    if wt_indices.is_empty() {
        return;
    }

    // Build tree nodes
    let mut nodes: Vec<TreeNode> = wt_indices
        .iter()
        .enumerate()
        .map(|(tree_idx, &wt_idx)| TreeNode {
            wt_idx,
            id: tree_idx + 1,
            parent_idx: None,
            base_sha: None,
            note: get_wt_note(&wts[wt_idx].path).unwrap_or_default(),
            children: Vec::new(),
        })
        .collect();

    let root_path = wts[wt_indices[0]].path.clone();

    // Assign parents by path containment
    for i in 1..nodes.len() {
        let child_parts: Vec<&str> = wts[nodes[i].wt_idx].path.split('/').collect();
        let mut best_parent = 0usize;
        let mut best_match: i64 = -1;

        for j in 0..i {
            let parent_name = wts[nodes[j].wt_idx].name();
            if let Some(pos) = child_parts[..child_parts.len().saturating_sub(1)]
                .iter()
                .rposition(|&p| p == parent_name)
            {
                if pos as i64 > best_match {
                    best_match = pos as i64;
                    best_parent = j;
                }
            }
        }

        nodes[i].parent_idx = Some(best_parent);

        // Compute merge-base
        let parent_sha = wts[nodes[best_parent].wt_idx].hash.clone();
        let child_sha = wts[nodes[i].wt_idx].hash.clone();
        let child_path = wts[nodes[i].wt_idx].path.clone();
        if let Ok(base) = crate::git::git_in(Some(&child_path), &["merge-base", &parent_sha, &child_sha]) {
            nodes[i].base_sha = Some(base);
        }
    }

    // Populate children lists
    for i in 1..nodes.len() {
        if let Some(p) = nodes[i].parent_idx {
            nodes[p].children.push(i);
        }
    }

    // Gather logs
    let logs: Vec<Vec<String>> = nodes.iter().map(|node| {
        let wt = &wts[node.wt_idx];
        get_logs(&wt.path, max_commits, node.base_sha.as_deref())
    }).collect();

    print_tree_node(0, &nodes, &wts, &logs, &root_path, "", true);
}
