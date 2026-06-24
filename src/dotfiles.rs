use crate::config::GwtpConfig;
use crate::git::git_toplevel;
use crate::sync::list_managed_files;
use crate::worktree::sorted_worktrees;
use std::path::Path;
use std::process::Command;

pub fn cmd_dotfiles(args: &[String], config: &GwtpConfig) {
    if args.is_empty() || args[0] != "edit" {
        eprintln!("Usage: gwtp dotfiles edit [path]");
        return;
    }

    let worktree_root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    let open_path: String = if args.len() >= 2 {
        resolve_to_worktree_copy(&args[1], &worktree_root)
    } else {
        let managed = list_managed_files(&worktree_root, config);
        if !managed.is_empty() && is_fzf_available() {
            let input = managed.join("\n");
            let selected = run_fzf(&input, "📄 Managed file> ");
            if selected.is_empty() {
                return;
            }
            Path::new(&worktree_root)
                .join(&selected)
                .to_string_lossy()
                .to_string()
        } else {
            // Default: open the worktree folder itself
            worktree_root.clone()
        }
    };

    eprintln!("Opening {} in {}", open_path, config.editor);
    Command::new(&config.editor)
        .arg(&open_path)
        .status()
        .unwrap_or_else(|e| {
            eprintln!("❌ Failed to open editor '{}': {}", config.editor, e);
            std::process::exit(1);
        });
}

/// Resolves the user-provided path to the equivalent file in the current worktree.
/// Relative paths are joined with worktree_root directly.
/// Absolute paths already inside the worktree are used as-is.
/// Absolute paths in a different worktree have their worktree prefix swapped.
fn resolve_to_worktree_copy(input: &str, worktree_root: &str) -> String {
    let p = Path::new(input);
    if !p.is_absolute() {
        return Path::new(worktree_root)
            .join(p)
            .to_string_lossy()
            .to_string();
    }
    if p.starts_with(worktree_root) {
        return input.to_string();
    }
    // Try to strip a known worktree root prefix and re-join with ours
    let wts = sorted_worktrees();
    for wt in &wts {
        if p.starts_with(&wt.path) {
            if let Ok(rel) = p.strip_prefix(&wt.path) {
                return Path::new(worktree_root)
                    .join(rel)
                    .to_string_lossy()
                    .to_string();
            }
        }
    }
    // Fallback: use as-is
    input.to_string()
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
        .args(["--no-sort", "--prompt", prompt, "--height=~40%", "--border"])
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
    let out = child.wait_with_output().unwrap_or_else(|_| std::process::exit(1));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}
