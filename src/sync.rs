use crate::config::load_config;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn sync_worktree_files(source: &str, target: &str, force: bool, common_git_dir: &str) {
    eprintln!("⚙️  Syncing environment files and configs to: {}", target);

    sync_env_files(source, target, force);

    let config = load_config(common_git_dir);
    if !config.sync_patterns.is_empty() {
        sync_via_patterns(source, target, &config.sync_patterns, force, "config patterns");
    }

    // Backwards compat: read .worktree.config if present
    let wt_config = Path::new(source).join(".worktree.config");
    if wt_config.exists() {
        sync_via_patterns(
            source,
            target,
            &[wt_config.to_string_lossy().to_string()],
            force,
            ".worktree.config",
        );
    }
}

fn sync_env_files(source: &str, target: &str, force: bool) {
    let files = find_env_files(Path::new(source));
    for file in files {
        let rel = file.strip_prefix(source).unwrap_or(&file);
        let dest = Path::new(target).join(rel);
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !dest.exists() || force {
            match fs::copy(&file, &dest) {
                Ok(_) => eprintln!("   ✅ Copied: {}", rel.display()),
                Err(e) => eprintln!("   ❌ Failed {}: {}", rel.display(), e),
            }
        } else {
            eprintln!("   ⏭️  Skipped (exists): {}", rel.display());
        }
    }
}

fn find_env_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    collect_env_files(dir, &mut results, 0);
    results
}

fn collect_env_files(dir: &Path, results: &mut Vec<PathBuf>, depth: u32) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name_str.as_ref(), "node_modules" | ".git" | "vendor" | ".next" | "target") {
                continue;
            }
            collect_env_files(&path, results, depth + 1);
        } else if name_str.starts_with(".env") {
            results.push(path);
        }
    }
}

pub fn sync_via_patterns(
    source: &str,
    target: &str,
    patterns: &[String],
    force: bool,
    label: &str,
) {
    use std::io::Write;
    let tmp = std::env::temp_dir().join("gwtp_exclude_tmp.gitignore");
    {
        let Ok(mut f) = fs::File::create(&tmp) else { return };
        for p in patterns {
            let _ = writeln!(f, "{}", p);
        }
    }

    let out = Command::new("git")
        .current_dir(source)
        .args(["ls-files", "--others", "--ignored", "--exclude-from"])
        .arg(&tmp)
        .output();

    let _ = fs::remove_file(&tmp);

    let Ok(o) = out else { return };
    if !o.status.success() {
        return;
    }

    let s = String::from_utf8_lossy(&o.stdout);
    let mut copied = 0usize;
    let mut skipped = 0usize;

    for file in s.lines() {
        if file.is_empty() {
            continue;
        }
        let src = Path::new(source).join(file);
        let dst = Path::new(target).join(file);
        if !src.is_file() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !dst.exists() || force {
            if fs::copy(&src, &dst).is_ok() {
                copied += 1;
            }
        } else {
            skipped += 1;
        }
    }

    if copied > 0 {
        eprintln!("   ✅ Copied {} file(s) ({})", copied, label);
    }
    if skipped > 0 {
        eprintln!("   ⏭️  Skipped {} file(s) (already exist)", skipped);
    }
}

pub fn copy_paths_to(source: &str, target: &str, abs_paths: &[PathBuf]) {
    for abs in abs_paths {
        let rel = abs.strip_prefix(source).unwrap_or(abs);
        let dst = Path::new(target).join(rel);
        if abs.is_dir() {
            let _ = fs::create_dir_all(&dst);
            copy_dir_recursive(abs, &dst);
            eprintln!("   ✅ Copied dir: {}", rel.display());
        } else {
            if let Some(parent) = dst.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::copy(abs, &dst) {
                Ok(_) => eprintln!("   ✅ Copied: {}", rel.display()),
                Err(e) => eprintln!("   ❌ Failed {}: {}", rel.display(), e),
            }
        }
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    let Ok(entries) = fs::read_dir(src) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let dest = dst.join(entry.file_name());
        if path.is_dir() {
            let _ = fs::create_dir_all(&dest);
            copy_dir_recursive(&path, &dest);
        } else {
            let _ = fs::copy(&path, &dest);
        }
    }
}
