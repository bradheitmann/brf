// File names. One convention, parsed and produced in one place.
//
//   brf_<repo-name>_<yyyymmdd>.html          a project brief
//   meta_brf_<yyyymmdd>.html                 the meta brief across all registered projects
//   ...<yyyymmdd>-v<N>...                    an earlier brief from the same day, in the archive
//   ..._<variant>.<ext>                      another format of the same brief, e.g. _dark.pdf
//
// <repo-name> is lower case, words joined by hyphens. Underscores only separate fields.
use regex::Regex;
use std::sync::LazyLock;

/// The meta brief's history key. It contains a character no repository name can, so a
/// project that happens to be called "meta" never shares numbering or filing with it.
pub const META_KEY: &str = "@meta";

pub static REPO_NAME_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9]+(?:-[a-z0-9]+)*$").unwrap());
const SEG: &str = "[a-z0-9]+(?:-[a-z0-9]+)*";
static PROJECT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^brf_({SEG})_([0-9]{{8}})(?:-v([0-9]+))?(?:_({SEG}))?\.([a-z0-9]+)$")).unwrap());
static META_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^meta_brf_([0-9]{{8}})(?:-v([0-9]+))?(?:_({SEG}))?\.([a-z0-9]+)$")).unwrap());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Project,
    Meta,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Project => "project",
            Kind::Meta => "meta",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub kind: Kind,
    /// The repository name, or META_KEY for the meta brief.
    pub key: String,
    pub repo: Option<String>,
    pub date: String,
    pub v: Option<u64>,
    pub variant: Option<String>,
    pub ext: String,
}

impl Parsed {
    /// A same-day copy number other than none or zero.
    pub fn has_v(&self) -> bool {
        self.v.is_some_and(|v| v != 0)
    }
}

pub fn is_real_date(yyyymmdd: &str) -> bool {
    if yyyymmdd.len() != 8 || !yyyymmdd.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let y: i64 = yyyymmdd[0..4].parse().unwrap_or(0);
    let m: i64 = yyyymmdd[4..6].parse().unwrap_or(0);
    let d: i64 = yyyymmdd[6..8].parse().unwrap_or(0);
    // Years 0 to 99 never round-trip (the 1.x date arithmetic read them as 1900 to 1999).
    if y < 100 || !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][(m - 1) as usize];
    d <= days
}

fn number(s: Option<regex::Match<'_>>) -> Option<u64> {
    s.map(|m| m.as_str().parse::<u64>().unwrap_or(u64::MAX))
}

pub fn parse_name(base: &str) -> Option<Parsed> {
    if let Some(c) = PROJECT_RE.captures(base) {
        let date = c[2].to_string();
        if !is_real_date(&date) {
            return None;
        }
        let repo = c[1].to_string();
        return Some(Parsed {
            kind: Kind::Project,
            key: repo.clone(),
            repo: Some(repo),
            date,
            v: number(c.get(3)),
            variant: c.get(4).map(|m| m.as_str().to_string()),
            ext: c[5].to_string(),
        });
    }
    if let Some(c) = META_RE.captures(base) {
        let date = c[1].to_string();
        if !is_real_date(&date) {
            return None;
        }
        return Some(Parsed {
            kind: Kind::Meta,
            key: META_KEY.into(),
            repo: None,
            date,
            v: number(c.get(2)),
            variant: c.get(3).map(|m| m.as_str().to_string()),
            ext: c[4].to_string(),
        });
    }
    None
}

pub fn format_name(kind: Kind, repo: Option<&str>, date: &str, v: Option<u64>, variant: Option<&str>, ext: &str) -> String {
    let stem = match kind {
        Kind::Meta => format!("meta_brf_{date}"),
        Kind::Project => format!("brf_{}_{date}", repo.unwrap_or("undefined")),
    };
    let v = match v {
        Some(n) if n != 0 => format!("-v{n}"),
        _ => String::new(),
    };
    let variant = match variant {
        Some(s) if !s.is_empty() => format!("_{s}"),
        _ => String::new(),
    };
    format!("{stem}{v}{variant}.{ext}")
}

/// A parsed name printed back, optionally with another same-day number.
pub fn format_parsed(p: &Parsed, v: Option<u64>) -> String {
    format_name(p.kind, p.repo.as_deref(), &p.date, v, p.variant.as_deref(), &p.ext)
}

/// A brief proper: the HTML page, not another format of it.
pub fn is_brief(parsed: Option<&Parsed>) -> bool {
    parsed.is_some_and(|p| p.ext == "html" && p.variant.is_none())
}

pub fn to_repo_name(raw: &str) -> String {
    let mut s = raw.to_lowercase();
    if let Some(stripped) = s.strip_suffix(".git") {
        s = stripped.to_string();
    }
    // Runs of whitespace, underscores and dots become one hyphen.
    let mut dashed = String::with_capacity(s.len());
    let mut in_run = false;
    for c in s.chars() {
        if crate::util::is_ws(c) || c == '_' || c == '.' {
            if !in_run {
                dashed.push('-');
            }
            in_run = true;
        } else {
            in_run = false;
            dashed.push(c);
        }
    }
    let kept: String = dashed.chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-').collect();
    let mut collapsed = String::with_capacity(kept.len());
    for c in kept.chars() {
        if c == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(c);
    }
    let s = collapsed.strip_prefix('-').unwrap_or(&collapsed);
    let s = s.strip_suffix('-').unwrap_or(s);
    s.to_string()
}

pub fn today_stamp() -> String {
    let (y, m, d) = crate::util::local_today();
    format!("{y:04}{m:02}{d:02}")
}

pub fn human_date(yyyymmdd: &str) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let day: u32 = yyyymmdd.get(6..8).and_then(|s| s.parse().ok()).unwrap_or(0);
    let month: usize = yyyymmdd.get(4..6).and_then(|s| s.parse().ok()).unwrap_or(0);
    let name = month.checked_sub(1).and_then(|i| MONTHS.get(i)).copied().unwrap_or("undefined");
    format!("{day} {name} {}", yyyymmdd.get(0..4).unwrap_or(""))
}

/// "20261102" as "2026-11-02".
pub fn dashed_date(yyyymmdd: &str) -> String {
    format!("{}-{}-{}", &yyyymmdd[0..4], &yyyymmdd[4..6], &yyyymmdd[6..8])
}

/// Days since 1970-01-01 for a yyyymmdd stamp.
pub fn days_ts(yyyymmdd: &str) -> i64 {
    let part = |r: std::ops::Range<usize>| yyyymmdd.get(r).and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    crate::util::days_from_civil(part(0..4), part(4..6), part(6..8))
}
