use std::process::Command;

#[derive(Debug, Clone)]
pub struct Worktree {
    pub path: String,
    pub hash: String,
    pub branch: String,
    pub is_bare: bool,
    pub is_detached: bool,
    pub is_main: bool,
    pub mtime: u64,
}

impl Worktree {
    pub fn name(&self) -> &str {
        std::path::Path::new(&self.path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.path)
    }

    pub fn display_branch(&self) -> String {
        if self.is_bare {
            "(bare)".to_string()
        } else if self.is_detached {
            format!("(detached:{})", &self.hash[..7.min(self.hash.len())])
        } else {
            format!("[{}]", self.branch)
        }
    }
}

pub fn list_worktrees_raw() -> Vec<Worktree> {
    let out = match Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };

    let s = String::from_utf8_lossy(&out.stdout);
    let mut result = Vec::new();
    let mut path = String::new();
    let mut hash = String::new();
    let mut branch = String::new();
    let mut is_bare = false;
    let mut is_detached = false;
    let mut is_first = true;

    let flush = |path: &str, hash: &str, branch: &str, is_bare: bool, is_detached: bool, is_main: bool, result: &mut Vec<Worktree>| {
        if !path.is_empty() {
            result.push(Worktree {
                path: path.to_string(),
                hash: hash.to_string(),
                branch: branch.to_string(),
                is_bare,
                is_detached,
                is_main,
                mtime: 0,
            });
        }
    };

    for line in s.lines() {
        if line.is_empty() {
            flush(&path, &hash, &branch, is_bare, is_detached, is_first, &mut result);
            if !path.is_empty() {
                is_first = false;
            }
            path.clear();
            hash.clear();
            branch.clear();
            is_bare = false;
            is_detached = false;
        } else if let Some(v) = line.strip_prefix("worktree ") {
            path = v.to_string();
        } else if let Some(v) = line.strip_prefix("HEAD ") {
            hash = v.to_string();
        } else if let Some(v) = line.strip_prefix("branch ") {
            branch = v.trim_start_matches("refs/heads/").to_string();
        } else if line == "bare" {
            is_bare = true;
        } else if line == "detached" {
            is_detached = true;
        }
    }
    flush(&path, &hash, &branch, is_bare, is_detached, is_first, &mut result);

    result
}

pub fn sorted_worktrees() -> Vec<Worktree> {
    let mut wts = list_worktrees_raw();
    if wts.len() <= 1 {
        return wts;
    }

    let main = wts.remove(0);

    for wt in &mut wts {
        let git_file = format!("{}/.git", wt.path);
        if let Ok(meta) = std::fs::metadata(&git_file) {
            if let Ok(modified) = meta.modified() {
                wt.mtime = modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
            }
        }
    }
    wts.sort_by_key(|w| w.mtime);

    let mut result = vec![main];
    result.extend(wts);
    result
}

pub fn is_worktree_hidden(wt: &Worktree, hidden_wt: &[String], hidden_br: &[String]) -> bool {
    if wt.is_main {
        return false;
    }
    let name = wt.name();
    for pfx in hidden_wt {
        if !pfx.is_empty() && name.starts_with(pfx.as_str()) {
            return true;
        }
    }
    for pfx in hidden_br {
        if !pfx.is_empty() && wt.branch.starts_with(pfx.as_str()) {
            return true;
        }
    }
    false
}

pub fn get_worktree_path(target: &str, wts: &[Worktree]) -> Option<String> {
    if let Ok(n) = target.parse::<usize>() {
        if n >= 1 && n <= wts.len() {
            return Some(wts[n - 1].path.clone());
        }
    }
    for wt in wts {
        if wt.branch == target {
            return Some(wt.path.clone());
        }
        if wt.name() == target {
            return Some(wt.path.clone());
        }
    }
    for wt in wts {
        if wt.path.contains(target) {
            return Some(wt.path.clone());
        }
    }
    None
}

pub fn get_wt_note(path: &str) -> Option<String> {
    let git_dir = crate::git::git_absolute_dir_in(path)?;
    let notes_file = std::path::Path::new(&git_dir).join("worktree_notes");
    std::fs::read_to_string(&notes_file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn get_wt_status(path: &str) -> String {
    let branch = match Command::new("git")
        .current_dir(path)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => return String::new(),
    };

    let has_remote = !branch.is_empty()
        && Command::new("git")
            .current_dir(path)
            .args(["ls-remote", "--exit-code", "--heads", "origin", &branch])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

    let status_out = match Command::new("git")
        .current_dir(path)
        .args(["status", "-sb"])
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return String::new(),
    };

    let mut lines = status_out.lines();
    let first_line = lines.next().unwrap_or("");

    let remote_info = if first_line.contains("...") {
        if first_line.contains('[') {
            let bracket_part = first_line.split('[').nth(1).unwrap_or("").trim_end_matches(']');
            let bracket_part = bracket_part
                .replace("ahead ", "❇️")
                .replace("behind ", "‼️");
            format!("🌐[{}]", bracket_part)
        } else {
            "🌐🔄".to_string()
        }
    } else if has_remote {
        "🌐".to_string()
    } else {
        String::new()
    };

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for line in lines {
        if line.len() >= 2 {
            let key = &line[..2];
            *counts.entry(key).or_insert(0) += 1;
        }
    }

    let file_info: Vec<String> = counts
        .iter()
        .map(|(k, v)| format!("{} {}", v, k))
        .collect();
    let file_str = file_info.join(" | ");

    match (remote_info.is_empty(), file_str.is_empty()) {
        (true, true) => String::new(),
        (true, false) => file_str,
        (false, true) => remote_info,
        (false, false) => format!("{} | {}", remote_info, file_str),
    }
}

pub fn get_wt_config_mtime(wt_path: &str) -> u64 {
    let mut latest: u64 = 0;
    collect_env_mtimes(std::path::Path::new(wt_path), &mut latest, 0);
    let config = std::path::Path::new(wt_path).join(".worktree.config");
    if config.exists() {
        if let Ok(m) = std::fs::metadata(&config) {
            if let Ok(t) = m.modified() {
                let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                if secs > latest {
                    latest = secs;
                }
            }
        }
    }
    latest
}

fn collect_env_mtimes(dir: &std::path::Path, latest: &mut u64, depth: u32) {
    if depth > 3 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name_str.as_ref(), "node_modules" | ".git" | "vendor" | ".next" | "target") {
                continue;
            }
            collect_env_mtimes(&path, latest, depth + 1);
        } else if name_str.starts_with(".env") {
            if let Ok(m) = std::fs::metadata(&path) {
                if let Ok(t) = m.modified() {
                    let secs = t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    if secs > *latest {
                        *latest = secs;
                    }
                }
            }
        }
    }
}

pub fn get_wt_config_date(wt_path: &str) -> Option<String> {
    let mtime = get_wt_config_mtime(wt_path);
    if mtime == 0 {
        return None;
    }
    use std::time::{Duration, UNIX_EPOCH};
    let t = UNIX_EPOCH + Duration::from_secs(mtime);
    let datetime: chrono_simple::Date = chrono_simple::unix_to_date(mtime);
    Some(format!("{:04}-{:02}-{:02}", datetime.year, datetime.month, datetime.day))
}

pub mod chrono_simple {
    pub struct Date {
        pub year: i32,
        pub month: u8,
        pub day: u8,
    }

    pub fn unix_to_date(secs: u64) -> Date {
        let days = secs / 86400;
        let mut y = 1970i32;
        let mut d = days as i64;
        loop {
            let days_in_year = if is_leap(y) { 366 } else { 365 };
            if d < days_in_year {
                break;
            }
            d -= days_in_year;
            y += 1;
        }
        let months = if is_leap(y) {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        let mut m = 0u8;
        for (i, &days_in_month) in months.iter().enumerate() {
            if d < days_in_month {
                m = (i + 1) as u8;
                break;
            }
            d -= days_in_month;
        }
        Date { year: y, month: m, day: (d + 1) as u8 }
    }

    fn is_leap(y: i32) -> bool {
        (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
    }
}
