use crate::config::GwtpConfig;
use crate::git::{git_main_root, git_toplevel};
use crate::worktree::{get_wt_config_mtime, get_worktree_path, sorted_worktrees};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use termtree::Tree;

// ── Pattern storage (standalone gitignore-style flat file) ─────────────────

pub fn patterns_file_path(common_git_dir: &str) -> PathBuf {
    Path::new(common_git_dir).join("sideload_patterns")
}

pub fn load_patterns(common_git_dir: &str) -> Vec<String> {
    migrate_legacy_patterns(common_git_dir);
    let path = patterns_file_path(common_git_dir);
    let Ok(content) = fs::read_to_string(&path) else { return Vec::new() };
    content
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect()
}

fn migrate_legacy_patterns(common_git_dir: &str) {
    let path = patterns_file_path(common_git_dir);
    if path.exists() {
        return;
    }
    let legacy = crate::config::load_config(common_git_dir).legacy_sync_patterns;
    if legacy.is_empty() {
        return;
    }
    eprintln!(
        "🔄 Migrating {} legacy sync_patterns from gwtp.json to {}",
        legacy.len(),
        path.display()
    );
    write_patterns_file(&path, &legacy);
    let _ = crate::config::sync_managed_block(common_git_dir, &legacy);
}

fn write_patterns_file(path: &Path, patterns: &[String]) {
    let mut content = String::from("# gwtp sideload patterns — gitignore syntax, one pattern per line\n");
    for p in patterns {
        content.push_str(p);
        content.push('\n');
    }
    let _ = fs::write(path, content);
}

pub fn save_patterns(common_git_dir: &str, patterns: &[String]) -> Result<(), String> {
    write_patterns_file(&patterns_file_path(common_git_dir), patterns);
    crate::config::sync_managed_block(common_git_dir, patterns)
}

pub fn cmd_add_pattern(common_git_dir: &str, pattern: &str) {
    let mut patterns = load_patterns(common_git_dir);
    if patterns.iter().any(|p| p == pattern) {
        eprintln!("Pattern already exists: {}", pattern);
        return;
    }
    patterns.push(pattern.to_string());
    match save_patterns(common_git_dir, &patterns) {
        Ok(_) => eprintln!("✅ Added pattern: {}", pattern),
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

pub fn cmd_rm_pattern(common_git_dir: &str, pattern: &str) {
    let mut patterns = load_patterns(common_git_dir);
    let before = patterns.len();
    patterns.retain(|p| p != pattern);
    if patterns.len() == before {
        eprintln!("Pattern not found: {}", pattern);
        return;
    }
    match save_patterns(common_git_dir, &patterns) {
        Ok(_) => eprintln!("✅ Removed pattern: {}", pattern),
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

pub fn cmd_list_patterns(common_git_dir: &str) {
    let patterns = load_patterns(common_git_dir);
    if patterns.is_empty() {
        println!("(no sideload patterns configured — see `gwtp sideload edit`)");
        return;
    }
    for p in &patterns {
        println!("{}", p);
    }
}

pub fn cmd_edit(common_git_dir: &str, config: &GwtpConfig) {
    let path = patterns_file_path(common_git_dir);
    let existing = load_patterns(common_git_dir);
    if !path.exists() {
        write_patterns_file(&path, &existing);
    }
    let status = Command::new(&config.editor)
        .arg(path.to_string_lossy().as_ref())
        .status();
    if !status.map(|s| s.success()).unwrap_or(false) {
        eprintln!("❌ Failed to open editor '{}'.", config.editor);
        std::process::exit(1);
    }
    let patterns = load_patterns(common_git_dir);
    if let Err(e) = crate::config::sync_managed_block(common_git_dir, &patterns) {
        eprintln!("⚠️  Failed to update .git/info/exclude: {}", e);
    }
}

// ── File discovery shared by copy + list ────────────────────────────────────

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

fn ls_files_via_exclude(source: &str, exclude_file: &Path) -> Vec<String> {
    let out = Command::new("git")
        .current_dir(source)
        .args(["ls-files", "--others", "--ignored", "--exclude-from"])
        .arg(exclude_file)
        .output();
    let Ok(o) = out else { return Vec::new() };
    if !o.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect()
}

/// All files (relative paths) that sideload patterns + `.env*` + the legacy
/// `.worktree.config` manage for `root`. Shared by the copy commands and the
/// `list`/`list-all` views so they always agree on what's "sideloaded".
pub fn gather_sideload_files(root: &str, common_git_dir: &str) -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();

    for f in find_env_files(Path::new(root)) {
        if let Ok(rel) = f.strip_prefix(root) {
            set.insert(rel.to_string_lossy().to_string());
        }
    }

    let patterns_file = patterns_file_path(common_git_dir);
    if patterns_file.exists() {
        for f in ls_files_via_exclude(root, &patterns_file) {
            set.insert(f);
        }
    }

    let wt_config = Path::new(root).join(".worktree.config");
    if wt_config.exists() {
        for f in ls_files_via_exclude(root, &wt_config) {
            set.insert(f);
        }
    }

    set.into_iter().collect()
}

// ── Copying ──────────────────────────────────────────────────────────────────

pub fn sideload_worktree_files(source: &str, target: &str, force: bool, common_git_dir: &str) {
    eprintln!("⚙️  Sideloading environment files and configs to: {}", target);

    let files = gather_sideload_files(source, common_git_dir);
    let mut copied = 0usize;
    let mut skipped = 0usize;

    for rel in &files {
        let src = Path::new(source).join(rel);
        let dst = Path::new(target).join(rel);
        if !src.is_file() {
            continue;
        }
        if let Some(parent) = dst.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if !dst.exists() || force {
            match copy_preserving_mtime(&src, &dst) {
                Ok(_) => {
                    copied += 1;
                    eprintln!("   ✅ Copied: {}", rel);
                }
                Err(e) => eprintln!("   ❌ Failed {}: {}", rel, e),
            }
        } else {
            skipped += 1;
        }
    }

    if copied > 0 {
        eprintln!("   ✅ Copied {} file(s)", copied);
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
            match copy_preserving_mtime(abs, &dst) {
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
            let _ = copy_preserving_mtime(&path, &dest);
        }
    }
}

/// Copies a file and carries over the source's mtime. Plain `fs::copy` stamps
/// the destination with the copy time, which then masquerades as the file's
/// "last edited" time — this is what keeps that timestamp meaningful across
/// repeated `gwtp sideload` copies between worktrees.
fn copy_preserving_mtime(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::copy(src, dst)?;
    let mtime = filetime::FileTime::from_last_modification_time(&fs::metadata(src)?);
    filetime::set_file_mtime(dst, mtime)?;
    Ok(())
}

fn file_mtime_secs(path: &Path) -> Option<i64> {
    fs::metadata(path)
        .ok()
        .map(|m| filetime::FileTime::from_last_modification_time(&m).unix_seconds())
}

// ── Commands moved here from ops.rs (sideload-specific) ─────────────────────

/// Initialize (sideload) all visible worktrees from main root
pub fn cmd_init(show_all: bool, config: &GwtpConfig, common_git_dir: &str) {
    let main_root = git_main_root().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    if show_all {
        eprintln!("🔄 Sideloading into ALL worktrees (including hidden) from main root: {}", main_root);
    } else {
        eprintln!("🔄 Sideloading into visible worktrees from main root: {}", main_root);
    }

    let wts = sorted_worktrees();
    let mut count = 0usize;

    for wt in &wts {
        if wt.path == main_root {
            continue;
        }
        if !show_all
            && crate::worktree::is_worktree_hidden(
                wt,
                &config.hidden_wt_prefixes,
                &config.hidden_branch_prefixes,
            )
        {
            continue;
        }
        eprintln!("--------------------------------------------------------");
        sideload_worktree_files(&main_root, &wt.path, false, common_git_dir);
        count += 1;
    }

    eprintln!("--------------------------------------------------------");
    if count == 0 {
        eprintln!("ℹ️  No additional worktrees found to sideload into.");
    } else {
        eprintln!("🎉 Successfully sideloaded into {} worktree(s)!", count);
    }
}

/// Broadcast sideload files from source worktree to all others
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
            let (best, _) = wts
                .iter()
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
        eprintln!("🔄 Broadcasting sideload files from: {}", source_root);
        for wt in &wts {
            if wt.path == source_root {
                continue;
            }
            eprintln!("--------------------------------------------------------");
            sideload_worktree_files(&source_root, &wt.path, true, common_git_dir);
            count += 1;
        }
    } else {
        eprintln!("🔄 Broadcasting specified paths from: {}", source_root);
        let resolved: Vec<PathBuf> = extra_paths
            .iter()
            .filter_map(|p| {
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
            })
            .collect();

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
        eprintln!("ℹ️  No other worktrees to sideload to.");
    } else {
        eprintln!("🎉 Successfully sideloaded to {} worktree(s)!", count);
    }
}

/// Copy sideload files from specified worktree into current
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
    eprintln!("📥 Copying sideload files from: {}", source_path);
    eprintln!("   To (current worktree):     {}", current_root);
    sideload_worktree_files(&source_path, &current_root, false, common_git_dir);
    eprintln!("🎉 Done.");
}

/// Copy sideload files from current worktree to specified
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
    eprintln!("📤 Copying sideload files from (current worktree): {}", current_root);
    eprintln!("   To: {}", target_path);
    sideload_worktree_files(&current_root, &target_path, false, common_git_dir);
    eprintln!("🎉 Done.");
}

// ── Tree listing ─────────────────────────────────────────────────────────────

enum Entry {
    Dir(BTreeMap<String, Entry>),
    Leaf,
}

/// One distinct content version of a path: its hash, the (1-based) worktree
/// indices that hold it, and the earliest mtime seen for that content.
type FileVariant = (String, Vec<usize>, Option<i64>);

fn insert_leaf(map: &mut BTreeMap<String, Entry>, components: &[&str], leaf_label: &str) {
    if components.len() <= 1 {
        map.insert(leaf_label.to_string(), Entry::Leaf);
        return;
    }
    let entry = map
        .entry(components[0].to_string())
        .or_insert_with(|| Entry::Dir(BTreeMap::new()));
    if let Entry::Dir(child_map) = entry {
        insert_leaf(child_map, &components[1..], leaf_label);
    }
}

fn to_termtree(name: String, map: &BTreeMap<String, Entry>) -> Tree<String> {
    let leaves: Vec<Tree<String>> = map
        .iter()
        .map(|(k, v)| match v {
            Entry::Dir(m) => to_termtree(k.clone(), m),
            Entry::Leaf => Tree::new(k.clone()),
        })
        .collect();
    Tree::new(name).with_leaves(leaves)
}

fn hash6(abs_path: &Path) -> String {
    Command::new("git")
        .args(["hash-object", &abs_path.to_string_lossy()])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .map(|s| s.chars().take(6).collect())
        .unwrap_or_else(|| "??????".to_string())
}

/// `gwtp sideload list` — tree of files sideload patterns manage in the
/// current worktree.
pub fn cmd_list(common_git_dir: &str) {
    let root = git_toplevel().unwrap_or_else(|| {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    });

    let files = gather_sideload_files(&root, common_git_dir);
    if files.is_empty() {
        println!("(no sideloaded files found in this worktree)");
        return;
    }

    let mut root_map: BTreeMap<String, Entry> = BTreeMap::new();
    for rel in &files {
        let components: Vec<&str> = rel.split('/').collect();
        let filename = components.last().copied().unwrap_or(rel.as_str());
        let abs = Path::new(&root).join(rel);
        let label = match file_mtime_secs(&abs) {
            Some(secs) => format!("{} ({})", filename, crate::date::format_datetime(secs)),
            None => filename.to_string(),
        };
        insert_leaf(&mut root_map, &components, &label);
    }

    println!("{}", to_termtree(root, &root_map));
    println!("\n{} file(s)", files.len());
}

/// `gwtp sideload list-all` — global tree across every worktree. Each unique
/// relative path appears once per distinct content hash, annotated with the
/// (1-based) worktree indices that hold that version.
pub fn cmd_list_all(common_git_dir: &str) {
    let wts = sorted_worktrees();
    if wts.is_empty() {
        eprintln!("❌ Not in a git repository.");
        std::process::exit(1);
    }

    // rel path -> [(hash, [worktree indices], earliest mtime seen for this content)]
    let mut file_variants: BTreeMap<String, Vec<FileVariant>> = BTreeMap::new();

    for (i, wt) in wts.iter().enumerate() {
        let idx = i + 1;
        for rel in gather_sideload_files(&wt.path, common_git_dir) {
            let abs = Path::new(&wt.path).join(&rel);
            let hash = hash6(&abs);
            let mtime = file_mtime_secs(&abs);
            let variants = file_variants.entry(rel).or_default();
            match variants.iter_mut().find(|(h, _, _)| *h == hash) {
                Some((_, indices, earliest)) => {
                    indices.push(idx);
                    // Identical content should carry the same (preserved) mtime;
                    // if copies predate mtime preservation, the earliest mtime
                    // seen is the closest approximation of the true edit time.
                    *earliest = match (*earliest, mtime) {
                        (Some(a), Some(b)) => Some(a.min(b)),
                        (a, b) => a.or(b),
                    };
                }
                None => variants.push((hash, vec![idx], mtime)),
            }
        }
    }

    if file_variants.is_empty() {
        println!("(no sideloaded files found across any worktree)");
        return;
    }

    let mut root_map: BTreeMap<String, Entry> = BTreeMap::new();
    let mut total_versions = 0usize;
    for (rel, variants) in &file_variants {
        let components: Vec<&str> = rel.split('/').collect();
        let filename = components.last().copied().unwrap_or(rel.as_str());
        for (hash, indices, mtime) in variants {
            let indices_str = indices
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let date_str = mtime
                .map(crate::date::format_datetime)
                .unwrap_or_else(|| "unknown".to_string());
            let label = format!("{} ({}, {}) [{}]", filename, hash, date_str, indices_str);
            insert_leaf(&mut root_map, &components, &label);
            total_versions += 1;
        }
    }

    let root_label = format!("sideload files across {} worktree(s)", wts.len());
    println!("{}", to_termtree(root_label, &root_map));
    println!(
        "\n{} file version(s) across {} unique path(s)",
        total_versions,
        file_variants.len()
    );
}
