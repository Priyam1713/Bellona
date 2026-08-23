//! Campaign XIV-6 — the ecosystem seed: install real skill packs from any
//! git URL, list what you carry, remove what you don't trust anymore.
//!
//! Format = the proven markdown+frontmatter pack (Hermes/Claude-Code/
//! Superpowers compatible). Foreign packs are read directly — no conversion.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillEntry {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Directory relative to the skills root.
    pub dir: String,
}

pub fn default_root() -> PathBuf {
    PathBuf::from("armamentarium/local")
}

/// Minimal frontmatter parser: first `---` block of SKILL.md, `key: value`.
/// Tolerant by design — foreign packs must load even when imperfect.
pub fn parse_frontmatter(raw: &str) -> Result<(String, String, String), String> {
    let mut lines = raw.lines().skip_while(|l| l.trim() != "---");
    lines.next(); // consume opening ---
    let mut name = String::new();
    let mut version = String::from("0.0.0");
    let mut description = String::new();
    for line in lines {
        let trimmed = line.trim_end();
        if trimmed == "---" {
            break;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let v = v.trim().to_string();
            match k.trim() {
                "name" if !v.is_empty() => name = v,
                "version" if !v.is_empty() => version = v,
                "description" if !v.is_empty() => description = v,
                _ => {}
            }
        }
    }
    if name.is_empty() {
        return Err("frontmatter missing 'name'".into());
    }
    Ok((name, version, description))
}

/// Discover every SKILL.md under `root`, returning parsed entries.
pub fn scan(root: &Path) -> Vec<SkillEntry> {
    let mut out = Vec::new();
    walk(root, root, &mut out, 0);
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<SkillEntry>, depth: usize) {
    if depth > 6 {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            walk(root, &p, out, depth + 1);
        } else if p.file_name().map(|f| f == "SKILL.md").unwrap_or(false) {
            if let Ok(raw) = std::fs::read_to_string(&p) {
                if let Ok((name, version, description)) = parse_frontmatter(&raw) {
                    let rel = p
                        .parent()
                        .and_then(|d| d.strip_prefix(root).ok())
                        .map(|d| d.to_string_lossy().replace('\\', "/"))
                        .unwrap_or_default();
                    out.push(SkillEntry {
                        name,
                        version,
                        description,
                        dir: rel,
                    });
                }
            }
        }
    }
}

/// Install from a git URL (or local path) into the skills root.
/// Returns the installed entries.
pub fn install_from_git(url: &str, root: &Path) -> Result<Vec<SkillEntry>, String> {
    let tmp = root.join(format!(".clone-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;

    let out = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url, &tmp.to_string_lossy()])
        .output()
        .map_err(|e| format!("git spawn: {e}"))?;
    if !out.status.success() {
        let msg = format!("clone failed: {}", String::from_utf8_lossy(&out.stderr));
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(msg.trim().to_string());
    }

    // Move each discovered skill dir into root/skills/<name>.
    let found = scan(&tmp);
    if found.is_empty() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("no SKILL.md packs found in source".into());
    }
    let mut installed = Vec::new();
    for entry in &found {
        let src = tmp.join(&entry.dir);
        let dest = root.join("skills").join(&entry.name);
        if dest.exists() {
            std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        copy_dir_recursive(&src, &dest)?;
        installed.push(SkillEntry {
            dir: format!("skills/{}", entry.name),
            ..entry.clone()
        });
    }
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(installed)
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let target = dest.join(entry.file_name());
        if p.is_dir() {
            copy_dir_recursive(&p, &target)?;
        } else {
            std::fs::copy(&p, &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Remove a skill pack by name (deletes its directory).
pub fn remove(root: &Path, name: &str) -> Result<bool, String> {
    let dir = root.join("skills").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
        Ok(true)
    } else {
        Ok(false)
    }
}
