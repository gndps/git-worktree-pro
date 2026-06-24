use crate::git::{git_current_branch, is_in_git_repo};
use crate::slug::{get_relative_path, get_slug, get_worktree_index};

/// Small prompt: "🌲 [N] branch" (or just "🌲 branch" for main WT)
pub fn cmd_prompt_small() {
    if !is_in_git_repo() {
        return;
    }
    let branch = git_current_branch().unwrap_or_default();
    let idx = get_worktree_index();

    match idx {
        Some(n) => print!("🌲 [{}] {}", n, branch),
        None => print!("🌲 {}", branch),
    }
}

/// Medium prompt: box format with slug, index, branch, relative path
pub fn cmd_prompt_medium() {
    if !is_in_git_repo() {
        return;
    }
    let slug = get_slug();
    let idx = get_worktree_index();
    let branch = git_current_branch().unwrap_or_default();
    let rel_path = get_relative_path().unwrap_or_default();

    let header = match idx {
        Some(n) => format!(" {} [{}] ", slug, n),
        None => format!(" {} ", slug),
    };

    let branch_line = format!(" 🌿 {} ", branch);
    let path_line = format!(" 📁 {} ", rel_path);

    let width = [header.len(), branch_line.len(), path_line.len()]
        .into_iter()
        .max()
        .unwrap_or(20);

    let border = "─".repeat(width);
    println!("┌{}┐", border);
    println!("│{:<width$}│", header, width = width);
    println!("│{:<width$}│", branch_line, width = width);
    if !rel_path.is_empty() {
        println!("│{:<width$}│", path_line, width = width);
    }
    println!("└{}┘", border);
}

/// Get prompt info as JSON for oh-my-posh or other integrations
pub fn cmd_prompt_json() {
    if !is_in_git_repo() {
        println!("{{}}");
        return;
    }
    let slug = get_slug();
    let idx = get_worktree_index();
    let branch = git_current_branch().unwrap_or_default();
    let rel_path = get_relative_path().unwrap_or_default();

    let index_part = match idx {
        Some(n) => format!(", \"index\": {}", n),
        None => String::new(),
    };
    println!(
        "{{\"slug\": \"{}\", \"branch\": \"{}\", \"path\": \"{}\"{}}}",
        escape_json(&slug),
        escape_json(&branch),
        escape_json(&rel_path),
        index_part
    );
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
