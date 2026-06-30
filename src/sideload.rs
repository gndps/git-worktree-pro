use crate::config::GwtpConfig;
use crate::git::{git_main_root, git_toplevel};
use crate::worktree::{get_wt_config_mtime, get_worktree_path, sorted_worktrees};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::WalkBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use termtree::Tree;

/// Directories never worth descending into while looking for sideload-managed
/// files — dependency/build trees that can dwarf the rest of the repo.
const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", "vendor", ".next", "target", "dist", "build",
    ".venv", "venv", "__pycache__", ".cache", ".turbo",
];

// ── Pattern storage (standalone JSON file) ──────────────────────────────────

/// `sideload_and_ignore` patterns are sideloaded *and* mirrored into the
/// `.git/info/exclude` MANAGED BLOCK, so git also treats matched files as
/// ignored. `sideload_only` patterns are sideloaded but never written to
/// `.git/info/exclude`, so matched files still show up in `git status` —
/// useful when you want a file copied between worktrees without hiding it
/// from git.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SideloadPatterns {
    #[serde(default)]
    pub sideload_and_ignore: Vec<String>,
    #[serde(default)]
    pub sideload_only: Vec<String>,
}

impl SideloadPatterns {
    fn contains(&self, pattern: &str) -> bool {
        self.sideload_and_ignore.iter().any(|p| p == pattern)
            || self.sideload_only.iter().any(|p| p == pattern)
    }

    fn all(&self) -> impl Iterator<Item = &String> {
        self.sideload_and_ignore.iter().chain(self.sideload_only.iter())
    }

    fn is_empty(&self) -> bool {
        self.sideload_and_ignore.is_empty() && self.sideload_only.is_empty()
    }
}

pub fn patterns_file_path(common_git_dir: &str) -> PathBuf {
    Path::new(common_git_dir).join("sideload_patterns.json")
}

/// The single source of truth for what's sideloaded: `sideload_patterns.json`
/// alone. No other file is consulted, and nothing is migrated from older
/// formats — if you had patterns somewhere else, re-add them with
/// `gwtp sideload add`.
pub fn load_patterns(common_git_dir: &str) -> SideloadPatterns {
    let path = patterns_file_path(common_git_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_patterns_file(path: &Path, patterns: &SideloadPatterns) {
    if let Ok(content) = serde_json::to_string_pretty(patterns) {
        let _ = fs::write(path, content + "\n");
    }
}

pub fn save_patterns(common_git_dir: &str, patterns: &SideloadPatterns) -> Result<(), String> {
    write_patterns_file(&patterns_file_path(common_git_dir), patterns);
    crate::config::sync_managed_block(common_git_dir, &patterns.sideload_and_ignore)
}

pub fn cmd_add_pattern(common_git_dir: &str, pattern: &str, only: bool) {
    let mut patterns = load_patterns(common_git_dir);
    if patterns.contains(pattern) {
        eprintln!("Pattern already exists: {}", pattern);
        return;
    }
    if only {
        patterns.sideload_only.push(pattern.to_string());
    } else {
        patterns.sideload_and_ignore.push(pattern.to_string());
    }
    match save_patterns(common_git_dir, &patterns) {
        Ok(_) => eprintln!(
            "✅ Added {} pattern: {}",
            if only { "sideload-only" } else { "sideload+ignore" },
            pattern
        ),
        Err(e) => eprintln!("❌ Error: {}", e),
    }
}

pub fn cmd_rm_pattern(common_git_dir: &str, pattern: &str) {
    let mut patterns = load_patterns(common_git_dir);
    if !patterns.contains(pattern) {
        eprintln!("Pattern not found: {}", pattern);
        return;
    }
    patterns.sideload_and_ignore.retain(|p| p != pattern);
    patterns.sideload_only.retain(|p| p != pattern);
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
    println!("sideload_and_ignore (mirrored into .git/info/exclude):");
    for p in &patterns.sideload_and_ignore {
        println!("  {}", p);
    }
    println!("sideload_only (kept out of .git/info/exclude, visible in git status):");
    for p in &patterns.sideload_only {
        println!("  {}", p);
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
    if let Err(e) = crate::config::sync_managed_block(common_git_dir, &patterns.sideload_and_ignore) {
        eprintln!("⚠️  Failed to update .git/info/exclude: {}", e);
    }
}

/// `gwtp sideload exclude` — explicitly re-sync the MANAGED BLOCK in
/// `.git/info/exclude` from `sideload_and_ignore` patterns. `add`/`rm`/
/// `edit` already do this; this command is for when the patterns file was
/// edited some other way (or you just want to confirm it's in sync).
pub fn cmd_exclude(common_git_dir: &str) {
    let patterns = load_patterns(common_git_dir);
    match crate::config::sync_managed_block(common_git_dir, &patterns.sideload_and_ignore) {
        Ok(_) => eprintln!(
            "✅ Synced {} pattern(s) into .git/info/exclude",
            patterns.sideload_and_ignore.len()
        ),
        Err(e) => {
            eprintln!("❌ Failed to update .git/info/exclude: {}", e);
            std::process::exit(1);
        }
    }
}

// ── File discovery shared by copy + list ────────────────────────────────────
//
// The old implementation shelled out to `git ls-files --others --ignored
// --exclude-from=<patterns>` per pattern source, per worktree. That flag is
// *additive* on top of .gitignore/.git/info/exclude/core.excludesFile — so it
// actually returned every git-ignored file in the tree (node_modules,
// target/, dist/, ...), not just files matching our sideload patterns. That
// both produced wrong results and, combined with a `git hash-object` spawn
// per matched file in `list-all`, made it spend most of its time forking
// processes for files nobody asked to sideload.
//
// This version matches only our own patterns in-process with the `ignore`
// crate (no git subprocess per file), walks each worktree once while pruning
// known dependency/build directories, and asks git which paths are tracked
// with a single `git ls-files` call per worktree so already-tracked files
// aren't pulled in by an overly broad pattern.

/// Builds an in-memory gitignore-style matcher from `sideload_patterns.json`
/// alone — both pattern lists (`sideload_and_ignore` + `sideload_only`) are
/// sideloaded the same way, they only differ in whether they're mirrored to
/// `.git/info/exclude`.
fn build_pattern_matcher(root: &str, common_git_dir: &str) -> Gitignore {
    let mut builder = GitignoreBuilder::new(root);
    for p in load_patterns(common_git_dir).all() {
        let _ = builder.add_line(None, p);
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

/// Relative paths (POSIX-style) git considers tracked in `root`, via a single
/// `git ls-files` call — avoids a subprocess per candidate file.
fn tracked_files(root: &str) -> HashSet<String> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z"])
        .output();
    let Ok(o) = out else { return HashSet::new() };
    if !o.status.success() {
        return HashSet::new();
    }
    o.stdout
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect()
}

/// Every file under `root`, skipping known dependency/build directories.
/// A single bounded walk replaces the old per-pattern-source git subprocess.
fn walk_candidate_files(root: &str) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .standard_filters(false) // we apply our own patterns below, not .gitignore
        .hidden(false) // don't skip dotfiles like .env
        .follow_links(false)
        .filter_entry(|entry| {
            entry.depth() == 0
                || !SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
        })
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.into_path())
        .collect()
}

/// All files (relative paths) that `sideload_patterns.json` + `.env*` manage
/// for `root`. Shared by the copy commands and the `list`/`list-all` views
/// so they always agree on what's "sideloaded".
pub fn gather_sideload_files(root: &str, common_git_dir: &str) -> Vec<String> {
    let matcher = build_pattern_matcher(root, common_git_dir);
    let tracked = tracked_files(root);

    let mut files: Vec<String> = walk_candidate_files(root)
        .into_iter()
        .filter_map(|abs| {
            let rel = abs.strip_prefix(root).ok()?;
            let rel_str = rel.to_string_lossy().to_string();
            let is_env = abs
                .file_name()
                .map(|n| n.to_string_lossy().starts_with(".env"))
                .unwrap_or(false);
            if is_env {
                return Some(rel_str);
            }
            if tracked.contains(&rel_str) {
                return None; // already version-controlled, not "sideloaded"
            }
            // `matched()` only tests the file's own path; a directory
            // pattern like `local/` needs `matched_path_or_any_parents()`
            // to also catch every file beneath it (gitignore semantics:
            // an ignored directory implies everything inside it).
            if matcher.matched_path_or_any_parents(rel, false).is_ignore() {
                Some(rel_str)
            } else {
                None
            }
        })
        .collect();

    files.sort();
    files.dedup();
    files
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

/// One discovered (rel path, hash, mtime, worktree index) reading, before
/// it's merged into `FileVariant`s.
type FileReading = (String, String, Option<i64>, usize);

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

/// 6-char content fingerprint, hashed in-process (no subprocess spawn per
/// file — this used to shell out to `git hash-object` for every matched
/// file in every worktree, which dominated `list-all`'s runtime).
fn hash6(abs_path: &Path) -> String {
    match fs::read(abs_path) {
        Ok(bytes) => blake3::hash(&bytes).to_hex()[..6].to_string(),
        Err(_) => "??????".to_string(),
    }
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

    // Discover + hash every worktree's sideload files in parallel (file
    // discovery, hashing, and mtime reads are exactly the work that used to
    // serialize on subprocess spawns). rayon's collect() preserves the
    // source order, so worktree indices stay correctly ascending below.
    let per_worktree: Vec<Vec<FileReading>> = wts
        .par_iter()
        .enumerate()
        .map(|(i, wt)| {
            let idx = i + 1;
            gather_sideload_files(&wt.path, common_git_dir)
                .into_par_iter()
                .map(|rel| {
                    let abs = Path::new(&wt.path).join(&rel);
                    let hash = hash6(&abs);
                    let mtime = file_mtime_secs(&abs);
                    (rel, hash, mtime, idx)
                })
                .collect()
        })
        .collect();

    // rel path -> [(hash, [worktree indices], earliest mtime seen for this content)]
    let mut file_variants: BTreeMap<String, Vec<FileVariant>> = BTreeMap::new();

    for (rel, hash, mtime, idx) in per_worktree.into_iter().flatten() {
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
