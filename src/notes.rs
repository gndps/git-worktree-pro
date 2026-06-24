use crate::git::{git_absolute_dir, is_in_git_repo};
use std::path::Path;

/// wtn: Get or set a note for the current worktree.
/// Notes are stored in <git-dir>/worktree_notes
pub fn cmd_note(note_text: Option<String>) {
    if !is_in_git_repo() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }
    let git_dir = git_absolute_dir().unwrap_or_else(|| {
        eprintln!("❌ Could not determine .git directory.");
        std::process::exit(1);
    });
    let notes_file = Path::new(&git_dir).join("worktree_notes");

    match note_text {
        None => {
            if notes_file.exists() {
                let content = std::fs::read_to_string(&notes_file).unwrap_or_default();
                let trimmed = content.trim();
                if trimmed.is_empty() {
                    println!("No notes found for this worktree.");
                } else {
                    println!("{}", trimmed);
                }
            } else {
                println!("No notes found for this worktree.");
            }
        }
        Some(text) => {
            std::fs::write(&notes_file, format!("{}\n", text.trim())).unwrap_or_else(|e| {
                eprintln!("❌ Failed to save note: {}", e);
                std::process::exit(1);
            });
            eprintln!("✅ Note saved for this worktree.");
        }
    }
}
