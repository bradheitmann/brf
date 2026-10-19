// Filesystem and folder orchestration; content rules live in validation.
use crate::error::{Result, io_at};
use crate::folder::{ARCHIVE_DIR, Location, history, list_files, sorted_names};
use crate::naming::{Kind, META_KEY, days_ts, is_brief, parse_name, today_stamp};
use crate::registry::{REGISTRY_FILE, load_registry};
use crate::util::{basename, join};

// Existing callers retain the same paths, types, diagnostics and ordering.
pub use crate::validation::{
    Attr, DEFAULT_BANNED, Tag, allowlist_problems, check_html, decode_css_escapes, markup_problems, scan_markup, stamp_mismatch,
};

pub fn check_file(path: &str, banned_extra: &[String]) -> Result<Vec<String>> {
    let bytes = std::fs::read(path).map_err(|e| io_at(e, "open", path))?;
    Ok(check_html(&String::from_utf8_lossy(&bytes), &basename(path), banned_extra))
}

#[derive(Clone, Debug, Default)]
pub struct FolderReport {
    pub problems: Vec<String>,
    pub notes: Vec<String>,
    /// Current project briefs. The meta brief is not a project brief and is not counted.
    pub current_count: usize,
    pub registered_count: usize,
}

impl FolderReport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "problems": self.problems,
            "notes": self.notes,
            "current_count": self.current_count,
            "registered_count": self.registered_count,
        })
    }
}

pub fn check_folder(out_dir: &str, banned_extra: &[String], stale_days: i64, today: &str) -> Result<FolderReport> {
    let mut problems = Vec::new();
    let mut notes = Vec::new();
    let registry = load_registry(out_dir)?;
    let mut registered: Vec<String> = Vec::new();
    for p in &registry.projects {
        if !registered.contains(&p.repo_name) {
            registered.push(p.repo_name.clone());
        }
    }
    if !registry.exists {
        problems.push(format!("{REGISTRY_FILE} is missing; the folder may not be synced yet. Tell the owner; only the owner runs brf init"));
    }

    // Current briefs by key, in the order the keys first appear.
    let mut current: Vec<(String, Vec<String>)> = Vec::new();
    for entry in sorted_names(out_dir) {
        if entry.starts_with('.') || entry == REGISTRY_FILE {
            continue;
        }
        let path = join(&[out_dir, &entry]);
        let meta = std::fs::metadata(&path).map_err(|e| io_at(e, "stat", &path))?;
        if meta.is_dir() {
            if entry != ARCHIVE_DIR {
                notes.push(format!("unexpected folder in the output folder: {entry}"));
            }
            continue;
        }
        let Some(parsed) = parse_name(&entry) else {
            problems.push(format!("{entry}: not a brf file name"));
            continue;
        };
        if parsed.has_v() {
            problems.push(format!("{entry}: same-day earlier copies belong in {ARCHIVE_DIR}/"));
        }
        if parsed.kind == Kind::Project {
            let repo = parsed.repo.as_deref().unwrap_or("");
            if !registered.iter().any(|r| r == repo) {
                problems.push(format!("{entry}: \"{repo}\" is not in the registry"));
            }
        }
        if !is_brief(Some(&parsed)) {
            continue;
        }
        match current.iter_mut().find(|(k, _)| *k == parsed.key) {
            Some((_, list)) => list.push(entry.clone()),
            None => current.push((parsed.key.clone(), vec![entry.clone()])),
        }
        for p in check_file(&path, banned_extra)? {
            problems.push(format!("{entry}: {p}"));
        }
        let h = history(out_dir, &parsed.key);
        let top = h.iter().map(|f| f.number).max().unwrap_or(0);
        if let Some(mine) = h.iter().find(|f| f.entry.name == entry && f.entry.location == Location::Root) {
            if mine.number != top {
                problems.push(format!("{entry}: is brief {} but the archive holds brief {top}", mine.number));
            }
        }
        let age_days = days_ts(today) - days_ts(&parsed.date);
        if age_days > stale_days {
            notes.push(format!("{entry}: {age_days} days old"));
        }
    }
    for (key, list) in &current {
        if list.len() > 1 {
            problems.push(format!("{key}: {} current briefs in the root ({}); one belongs in {ARCHIVE_DIR}/", list.len(), list.join(", ")));
        }
    }
    for repo in &registered {
        if !current.iter().any(|(k, _)| k == repo) {
            notes.push(format!("{repo}: registered, no current brief"));
        }
    }
    let unnamed: Vec<String> =
        list_files(out_dir).into_iter().filter(|f| f.location == Location::Archive && f.parsed.is_none()).map(|f| f.name).collect();
    if !unnamed.is_empty() {
        notes.push(format!(
            "{} archive file(s) do not follow the naming: {}",
            unnamed.len(),
            unnamed.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(FolderReport { problems, notes, current_count: current.iter().filter(|(k, _)| k != META_KEY).count(), registered_count: registered.len() })
}

/// The folder check with today's date and the protocol's 14-day staleness.
pub fn check_folder_today(out_dir: &str, banned_extra: &[String]) -> Result<FolderReport> {
    check_folder(out_dir, banned_extra, 14, &today_stamp())
}
