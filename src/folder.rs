// The output folder: current briefs in the root, history in archive/.
//
//   <output>/registry.json
//   <output>/brf_<repo>_<yyyymmdd>.html     one current brief per project
//   <output>/meta_brf_<yyyymmdd>.html       one current meta brief
//   <output>/archive/                       every earlier brief, never deleted
use crate::html::read_stamp;
use crate::naming::{Parsed, is_brief, parse_name};
use crate::util::join;
use std::cmp::Ordering;

pub const ARCHIVE_DIR: &str = "archive";

pub fn archive_dir(out_dir: &str) -> String {
    join(&[out_dir, ARCHIVE_DIR])
}

pub fn ensure_layout(out_dir: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(archive_dir(out_dir))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Location {
    Root,
    Archive,
}

impl Location {
    pub fn as_str(self) -> &'static str {
        match self {
            Location::Root => "root",
            Location::Archive => "archive",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub location: Location,
    pub parsed: Option<Parsed>,
}

/// The names in a folder, sorted by byte value, without hidden files.
pub fn sorted_names(dir: &str) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn scan(dir: &str, location: Location) -> Vec<Entry> {
    let mut out = Vec::new();
    for name in sorted_names(dir) {
        if name.starts_with('.') {
            continue;
        }
        let path = join(&[dir, &name]);
        match std::fs::metadata(&path) {
            Ok(m) if m.is_file() => {}
            _ => continue,
        }
        let parsed = parse_name(&name);
        out.push(Entry { name, path, location, parsed });
    }
    out
}

pub fn list_files(out_dir: &str) -> Vec<Entry> {
    let mut all = scan(out_dir, Location::Root);
    all.extend(scan(&archive_dir(out_dir), Location::Archive));
    all
}

/// Oldest first: by date, then earlier same-day copies (-v1, -v2) before the plain name.
fn order(a: &Entry, b: &Entry) -> Ordering {
    let (pa, pb) = (a.parsed.as_ref().expect("brief"), b.parsed.as_ref().expect("brief"));
    if pa.date != pb.date {
        return pa.date.cmp(&pb.date);
    }
    let va = pa.v.unwrap_or(u64::MAX);
    let vb = pb.v.unwrap_or(u64::MAX);
    if va != vb {
        return va.cmp(&vb);
    }
    if a.location == Location::Archive { Ordering::Less } else { Ordering::Greater }
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub entry: Entry,
    pub number: u64,
    pub stamped: bool,
}

/// Every brief of one project (key = repo name, or META_KEY for the meta brief), oldest first,
/// each with its number. A stamped brief carries its own number; an unstamped one takes its position.
pub fn history(out_dir: &str, key: &str) -> Vec<HistoryEntry> {
    let mut briefs: Vec<Entry> =
        list_files(out_dir).into_iter().filter(|f| is_brief(f.parsed.as_ref()) && f.parsed.as_ref().is_some_and(|p| p.key == key)).collect();
    briefs.sort_by(order);
    let mut last = 0u64;
    briefs
        .into_iter()
        .enumerate()
        .map(|(i, entry)| {
            let stamped = std::fs::read(&entry.path).ok().and_then(|b| read_stamp(&String::from_utf8_lossy(&b)).brief);
            let number = stamped.unwrap_or_else(|| (last + 1).max(i as u64 + 1));
            last = last.max(number);
            HistoryEntry { entry, number, stamped: stamped.is_some() }
        })
        .collect()
}

pub fn next_number(out_dir: &str, key: &str) -> u64 {
    history(out_dir, key).iter().map(|h| h.number).max().unwrap_or(0).saturating_add(1)
}

pub fn current_files(out_dir: &str, key: &str) -> Vec<Entry> {
    scan(out_dir, Location::Root).into_iter().filter(|f| is_brief(f.parsed.as_ref()) && f.parsed.as_ref().is_some_and(|p| p.key == key)).collect()
}
