use crate::git::is_in_git_repo;
use crate::slug::{get_relative_path, get_slug, get_worktree_index};
use std::process::Command;

// Metal gradient: bright top, dark bottom (cycles every 6 rows)
const METAL_COLORS: &[&str] = &[
    "\x1b[97m",         // bright white
    "\x1b[38;5;189m",   // light lavender
    "\x1b[38;5;153m",   // light blue
    "\x1b[38;5;111m",   // cornflower blue
    "\x1b[38;5;69m",    // blue
    "\x1b[38;5;57m",    // deep indigo
];

const BORDER_COLOR: &str = "\x1b[38;5;63m";
const RESET: &str = "\x1b[0m";

pub struct PromptOpts {
    /// Show 🌲 tree emoji prefix
    pub emoji: bool,
    /// Use toilet rendering; Some(font) where "" means default "future" font
    pub toilet: Option<String>,
    /// Use figlet rendering; Some(font) where "" means default "standard" font
    pub figlet: Option<String>,
}

fn figlet_text() -> Option<String> {
    if !is_in_git_repo() {
        return None;
    }
    let slug = get_slug();
    let idx = get_worktree_index();
    Some(match idx {
        Some(n) => format!("{} - {}", n, slug),
        None => slug,
    })
}

// ── Small prompt ─────────────────────────────────────────────────────────────

pub fn cmd_prompt_small(emoji: bool) {
    if !is_in_git_repo() {
        return;
    }
    let text = match figlet_text() {
        Some(t) if !t.is_empty() => t,
        _ => return,
    };
    if emoji {
        print!("🌲 {}", text);
    } else {
        print!("{}", text);
    }
}

// ── Medium prompt (banner) ────────────────────────────────────────────────────

pub fn cmd_prompt_medium(opts: &PromptOpts) {
    if !is_in_git_repo() {
        return;
    }
    let text = match figlet_text() {
        Some(t) if !t.is_empty() => t,
        _ => return,
    };
    let rel_path = get_relative_path().unwrap_or_default();

    let prefix = if opts.emoji { "🌲 " } else { "" };

    let banner = if let Some(font) = &opts.toilet {
        render_toilet(&text, if font.is_empty() { "future" } else { font })
    } else if let Some(font) = &opts.figlet {
        render_figlet(&text, if font.is_empty() { "" } else { font })
    } else {
        // Default: native figlet + border + metal
        render_native_banner(&text)
    };

    if !prefix.is_empty() {
        println!("{}{}", prefix, banner.trim_start_matches('\n'));
    } else {
        println!("{}", banner);
    }
    if !rel_path.is_empty() {
        println!("{}", rel_path);
    }
}

// ── Toilet mode ───────────────────────────────────────────────────────────────
// Tries system toilet first; falls back to native rendering.

fn render_toilet(text: &str, font: &str) -> String {
    // Search for toilet font in system paths
    let toilet_font_dirs = [
        "/opt/homebrew/opt/toilet/share/figlet-fonts",
        "/opt/homebrew/share/figlet-fonts",
        "/usr/local/share/figlet-fonts",
        "/usr/share/figlet",
        "/usr/share/toilet",
    ];

    let has_font = toilet_font_dirs
        .iter()
        .any(|dir| std::path::Path::new(&format!("{}/{}.tlf", dir, font)).exists()
            || std::path::Path::new(&format!("{}/{}.flf", dir, font)).exists());

    if has_font || is_command_available("toilet") {
        let result = Command::new("toilet")
            .args(["-t", "-f", font, "-F", "border", "-F", "metal", text])
            .output();
        if let Ok(o) = result {
            if o.status.success() && !o.stdout.is_empty() {
                return String::from_utf8_lossy(&o.stdout).to_string();
            }
        }
    }

    // Fallback: native rendering
    render_native_banner(text)
}

// ── Figlet mode ───────────────────────────────────────────────────────────────
// Tries system figlet (or figlet-rs with a font path); falls back to bundled font.

fn render_figlet(text: &str, font: &str) -> String {
    // Try system figlet with the requested font
    if !font.is_empty() && is_command_available("figlet") {
        let result = Command::new("figlet")
            .args(["-t", "-f", font, text])
            .output();
        if let Ok(o) = result {
            if o.status.success() && !o.stdout.is_empty() {
                let ascii = String::from_utf8_lossy(&o.stdout).to_string();
                return add_border(&apply_metal(&ascii));
            }
        }
    }

    // Try loading a .flf font file from system paths
    if !font.is_empty() {
        let font_dirs = [
            "/opt/homebrew/opt/figlet/share/figlet/fonts",
            "/opt/homebrew/share/figlet",
            "/usr/local/share/figlet",
            "/usr/share/figlet",
        ];
        for dir in &font_dirs {
            let path = format!("{}/{}.flf", dir, font);
            if let Ok(data) = std::fs::read_to_string(&path) {
                if let Some(banner) = figlet_from_content(text, &data) {
                    return add_border(&apply_metal(&banner));
                }
            }
        }
    }

    // Fall back to native bundled standard font
    render_native_banner(text)
}

// ── Native banner (figlet-rs + border + metal) ────────────────────────────────

fn render_native_banner(text: &str) -> String {
    let ascii = native_figlet(text);
    add_border(&apply_metal(&ascii))
}

fn native_figlet(text: &str) -> String {
    use figlet_rs::FIGfont;
    if let Ok(font) = FIGfont::standard() {
        if let Some(fig) = font.convert(text) {
            return fig.to_string();
        }
    }
    format!(" {} \n", text)
}

fn figlet_from_content(text: &str, data: &str) -> Option<String> {
    use figlet_rs::FIGfont;
    let font = FIGfont::from_content(data).ok()?;
    Some(font.convert(text)?.to_string())
}

// ── Metal color gradient ──────────────────────────────────────────────────────

fn apply_metal(text: &str) -> String {
    text.lines()
        .enumerate()
        .map(|(i, line)| {
            let color = METAL_COLORS[i % METAL_COLORS.len()];
            format!("{}{}{}", color, line, RESET)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Border drawing ────────────────────────────────────────────────────────────

fn visual_len(s: &str) -> usize {
    let mut len = 0usize;
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
        } else {
            len += if ch.len_utf8() > 1 { 2 } else { 1 };
        }
    }
    len
}

fn add_border(colored_text: &str) -> String {
    let lines: Vec<&str> = colored_text.lines().collect();
    let max_w = lines.iter().map(|l| visual_len(l)).max().unwrap_or(0);
    let h = "═".repeat(max_w + 2);

    let mut out = format!("{}╔{}╗{}\n", BORDER_COLOR, h, RESET);
    for line in &lines {
        let vl = visual_len(line);
        let pad = " ".repeat(max_w.saturating_sub(vl));
        out.push_str(&format!(
            "{}║{} {}{} {}║{}\n",
            BORDER_COLOR, RESET, line, pad, BORDER_COLOR, RESET
        ));
    }
    out.push_str(&format!("{}╚{}╝{}", BORDER_COLOR, h, RESET));
    out
}

fn is_command_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ── JSON / machine-readable ───────────────────────────────────────────────────

pub fn cmd_prompt_json() {
    if !is_in_git_repo() {
        println!("{{}}");
        return;
    }
    let slug = get_slug();
    let idx = get_worktree_index();
    let rel_path = get_relative_path().unwrap_or_default();

    let index_part = match idx {
        Some(n) => format!(", \"index\": {}", n),
        None => String::new(),
    };
    println!(
        "{{\"slug\": \"{}\", \"path\": \"{}\"{}}}",
        escape_json(&slug),
        escape_json(&rel_path),
        index_part
    );
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}
