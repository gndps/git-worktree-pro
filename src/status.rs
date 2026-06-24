use crate::git::is_in_git_repo;
use std::process::Command;

/// gsd equivalent: compact git status with tree view
pub fn cmd_status() {
    if !is_in_git_repo() {
        eprintln!("Not a git repository.");
        std::process::exit(1);
    }

    let branch_out = Command::new("git")
        .args(["-c", "color.status=always", "status", "-sb"])
        .output();

    if let Ok(o) = branch_out {
        let s = String::from_utf8_lossy(&o.stdout);
        // Print first line (branch info)
        if let Some(first) = s.lines().next() {
            println!("{}", first);
        }
        println!();
    }

    let status_raw_out = Command::new("git")
        .args(["status", "--porcelain"])
        .output();

    let status_raw = match status_raw_out {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => {
            eprintln!("❌ Failed to get git status.");
            return;
        }
    };

    if status_raw.trim().is_empty() {
        println!("   \x1b[32m✓ Working tree clean\x1b[0m");
        return;
    }

    let mut mod_count = 0usize;
    let mut add_count = 0usize;
    let mut del_count = 0usize;
    let mut unt_count = 0usize;

    for line in status_raw.lines() {
        if line.len() < 2 {
            continue;
        }
        let code = &line[..2];
        match code {
            " M" | "M " | "MM" | "AM" => mod_count += 1,
            " A" | "A " | "AA" => add_count += 1,
            " D" | "D " | "DD" | "AD" => del_count += 1,
            "??" => unt_count += 1,
            _ => {}
        }
    }

    let mut stats_parts = Vec::new();
    if mod_count > 0 {
        stats_parts.push(format!("\x1b[33m{} mod\x1b[0m", mod_count));
    }
    if add_count > 0 {
        stats_parts.push(format!("\x1b[32m{} new\x1b[0m", add_count));
    }
    if unt_count > 0 {
        stats_parts.push(format!("\x1b[32m{} untracked\x1b[0m", unt_count));
    }
    if del_count > 0 {
        stats_parts.push(format!("\x1b[31m{} del\x1b[0m", del_count));
    }

    if !stats_parts.is_empty() {
        println!("\x1b[1mStats:\x1b[0m {}\n", stats_parts.join(" "));
    }

    // Simple listing (without tree — tree cmd not always available)
    for line in status_raw.lines() {
        if line.len() < 3 {
            continue;
        }
        let code = &line[..2];
        let file = &line[3..];
        let colored_code = match code {
            " M" | "M " | "MM" => format!("\x1b[33m[{}]\x1b[0m", code),
            " A" | "A " | "??" => format!("\x1b[32m[{}]\x1b[0m", code),
            " D" | "D " => format!("\x1b[31m[{}]\x1b[0m", code),
            " R" | "R " => format!("\x1b[35m[{}]\x1b[0m", code),
            _ => format!("[{}]", code),
        };
        println!("  {} {}", colored_code, file);
    }
    println!();
}
