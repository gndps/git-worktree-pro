use crate::worktree::{get_worktree_path, sorted_worktrees};
use crate::git::git_toplevel;
use crate::config::GwtpConfig;
use std::process::Command;

/// wcd: Navigate to worktree, preserving relative path.
/// Prints `cd '<path>'` to stdout for shell eval; all other output to stderr.
pub fn cmd_cd(target: &str) {
    let wts = sorted_worktrees();
    let target_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ No worktree found for '{}'.", target);
        std::process::exit(1);
    });

    let repo_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let rel_path = cwd.strip_prefix(&repo_root).unwrap_or("").to_string();
    let dest = format!("{}{}", target_path, rel_path);
    let dest_path = std::path::Path::new(&dest);

    let final_dest = if dest_path.is_dir() {
        dest
    } else {
        target_path
    };

    println!("cd '{}'", final_dest);
}

/// wwi: Open editor at worktree, preserving relative path
pub fn cmd_open(target: &str, config: &GwtpConfig) {
    let wts = sorted_worktrees();
    let target_path = get_worktree_path(target, &wts).unwrap_or_else(|| {
        eprintln!("❌ No worktree found for '{}'.", target);
        std::process::exit(1);
    });

    let repo_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    let cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let rel_path = cwd.strip_prefix(&repo_root).unwrap_or("").to_string();
    let dest = format!("{}{}", target_path, rel_path);
    let open_path = if std::path::Path::new(&dest).is_dir() {
        &dest
    } else {
        &target_path
    };

    eprintln!("Opening {} at: {}", config.editor, open_path);
    Command::new(&config.editor).arg(open_path).status().ok();
}

/// wwif: fzf picker to select worktree, then open in editor
pub fn cmd_open_pick(show_all: bool, config: &GwtpConfig) {
    use crate::worktree::{get_wt_note, is_worktree_hidden};
    use crate::list::{YELLOW, CYAN_BOLD, DIM, SKY_BLUE, GRAY, RESET};

    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }

    if !is_fzf_available() {
        eprintln!("❌ fzf is not installed. Install it with: brew install fzf");
        std::process::exit(1);
    }

    let current_root = git_toplevel().unwrap_or_default();

    // Build fzf input lines: "<index>\t<display>"
    let mut lines: Vec<String> = Vec::new();
    let mut i = 1usize;
    for wt in &wts {
        if !show_all && i > 1 && is_worktree_hidden(wt, &config.hidden_wt_prefixes, &config.hidden_branch_prefixes) {
            i += 1;
            continue;
        }
        let marker = if wt.path == current_root { "*" } else { " " };
        let bd = wt.display_branch();
        let note = get_wt_note(&wt.path).unwrap_or_default();
        let note_part = if note.is_empty() {
            String::new()
        } else {
            format!("  ||  {}☁️  {}{}", SKY_BLUE, note, RESET)
        };
        let display = format!(
            "{}[{}{}]{}  ||  {}{}{}  ||  {}{}{}{}",
            YELLOW, marker, i, RESET,
            CYAN_BOLD, bd, RESET,
            DIM, wt.path, RESET,
            note_part
        );
        lines.push(format!("{}\t{}", i, display));
        i += 1;
    }

    if lines.is_empty() {
        eprintln!("ℹ️  No worktrees to display.");
        return;
    }

    // Run fzf with input via stdin
    let input = lines.join("\n");
    let selected = run_fzf(&input, "🌿 Worktree> ");
    if selected.is_empty() {
        return;
    }

    let idx_str = selected.split('\t').next().unwrap_or("").trim();
    if let Ok(idx) = idx_str.parse::<usize>() {
        if idx >= 1 && idx <= wts.len() {
            let wt_path = &wts[idx - 1].path;
            eprintln!("Opening {} at: {}", config.editor, wt_path);
            Command::new(&config.editor).arg(wt_path).status().ok();
        }
    }
}

fn is_fzf_available() -> bool {
    Command::new("fzf")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn run_fzf(input: &str, prompt: &str) -> String {
    use std::io::Write;
    let mut child = Command::new("fzf")
        .args(["--ansi", "--no-sort", "--delimiter=\t", "--with-nth=2..",
               "--prompt", prompt, "--height=~50%", "--border"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| {
            eprintln!("❌ Failed to launch fzf.");
            std::process::exit(1);
        });

    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(input.as_bytes());
    }

    let out = child.wait_with_output().unwrap_or_else(|_| {
        std::process::exit(1);
    });

    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// wcp: Copy worktree folder name to clipboard
pub fn cmd_copy_name(index: usize) {
    let wts = sorted_worktrees();
    if index < 1 || index > wts.len() {
        eprintln!("❌ No worktree at index {}.", index);
        std::process::exit(1);
    }
    let name = wts[index - 1].name().to_string();
    copy_to_clipboard(&name);
}

fn copy_to_clipboard(s: &str) {
    let cmds = [
        ("pbcopy", vec![]),
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("clip.exe", vec![]),
    ];
    for (cmd, args) in &cmds {
        if Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .ok()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.as_mut()?.write_all(s.as_bytes()).ok()?;
                Some(c)
            })
            .and_then(|mut c| c.wait().ok())
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!("📋 Copied \x1b[1;36m{}\x1b[0m to clipboard.", s);
            return;
        }
    }
    println!("{}", s);
}
