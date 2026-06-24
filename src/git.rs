use std::process::Command;

pub fn git(args: &[&str]) -> Result<String, String> {
    git_in(None, args)
}

pub fn git_in(dir: Option<&str>, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    cmd.args(args);
    let out = cmd.output().map_err(|e| format!("git exec: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim_end().to_string())
    }
}

pub fn git_toplevel() -> Option<String> {
    git(&["rev-parse", "--show-toplevel"]).ok()
}

pub fn git_main_root() -> Option<String> {
    let out = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    for line in s.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            return Some(path.to_string());
        }
    }
    None
}

pub fn git_common_dir() -> Option<String> {
    git(&["rev-parse", "--git-common-dir"]).ok()
}

pub fn git_dir() -> Option<String> {
    git(&["rev-parse", "--git-dir"]).ok()
}

pub fn git_absolute_dir() -> Option<String> {
    git(&["rev-parse", "--absolute-git-dir"]).ok()
}

pub fn git_absolute_dir_in(path: &str) -> Option<String> {
    git_in(Some(path), &["rev-parse", "--absolute-git-dir"]).ok()
}

pub fn git_current_branch() -> Option<String> {
    git(&["rev-parse", "--abbrev-ref", "HEAD"]).ok()
}

pub fn is_in_git_repo() -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
