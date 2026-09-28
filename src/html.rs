// Small helpers for reading finished pages.
use crate::util::{WS, squash_ws};
use regex::Regex;
use std::sync::LazyLock;

pub fn read_meta(html: &str, name: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"<meta{WS}+name="{}"{WS}+content="([^"]*)"{WS}*/?>"#, regex::escape(name))).ok()?;
    re.captures(html).map(|c| c[1].to_string())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stamp {
    pub brf_version: Option<String>,
    pub template: Option<String>,
    pub brief: Option<u64>,
    pub repo: Option<String>,
    pub date: Option<String>,
}

pub fn read_stamp(html: &str) -> Stamp {
    let brief = read_meta(html, "brf-brief");
    Stamp {
        brf_version: read_meta(html, "brf-version"),
        template: read_meta(html, "brf-template"),
        brief: brief
            .filter(|b| !b.is_empty() && b.bytes().all(|c| c.is_ascii_digit()))
            .map(|b| b.parse::<u64>().unwrap_or(u64::MAX)),
        repo: read_meta(html, "brf-repo"),
        date: read_meta(html, "brf-date"),
    }
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "nbsp" => " ",
        "middot" => "·",
        "rsquo" => "\u{2019}",
        "lsquo" => "\u{2018}",
        "ldquo" => "\u{201C}",
        "rdquo" => "\u{201D}",
        "mdash" => "\u{2014}",
        "ndash" => "\u{2013}",
        "hellip" => "\u{2026}",
        _ => return None,
    })
}

static ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(#[xX][0-9a-fA-F]+|#[0-9]+|[a-zA-Z][a-zA-Z0-9]*);?").unwrap());

pub fn decode_entities(s: &str) -> String {
    ENTITY_RE
        .replace_all(s, |c: &regex::Captures<'_>| {
            let all = &c[0];
            let e = &c[1];
            if let Some(num) = e.strip_prefix('#') {
                let code = if let Some(h) = num.strip_prefix(['x', 'X']) { u32::from_str_radix(h, 16).ok() } else { num.parse::<u32>().ok() };
                return match code.and_then(char::from_u32) {
                    Some(ch) if ch != '\0' => ch.to_string(),
                    _ => "\u{FFFD}".to_string(),
                };
            }
            if !all.ends_with(';') {
                return all.to_string();
            }
            named_entity(e).map(str::to_string).unwrap_or_else(|| all.to_string())
        })
        .into_owned()
}

static STYLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<style)(?s:.)*?(?i-u:</style>)").unwrap());
static SCRIPT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<script)(?s:.)*?(?i-u:</script>)").unwrap());
static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--(?s:.)*?-->").unwrap());
static HEAD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<head)(?s:.)*?(?i-u:</head>)").unwrap());
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());

pub fn visible_text(html: &str) -> String {
    let s = STYLE_RE.replace_all(html, " ");
    let s = SCRIPT_RE.replace_all(&s, " ");
    let s = COMMENT_RE.replace_all(&s, " ");
    let s = HEAD_RE.replace(&s, " ");
    let s = TAG_RE.replace_all(&s, " ");
    squash_ws(&decode_entities(&s))
}

static APPENDIX_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<section[^>]*data-brf="appendix""#).unwrap());
static FOOTER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<footer(?-u:\b)").unwrap());
static LEGACY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"<h2>{WS}*Planning records{WS}*</h2>")).unwrap());

/// The part of a page the reader must get through: everything before the appendices
/// (marked data-brf="appendix") and before the footer.
pub fn main_body(html: &str) -> &str {
    let appendix = APPENDIX_RE.find(html).map(|m| m.start());
    let footer = FOOTER_RE.find(html).map(|m| m.start());
    let cut = [appendix, footer].into_iter().flatten().min().unwrap_or(html.len());
    let mut body = &html[..cut];
    // Briefs built before the appendix marker existed: fall back to the heading text.
    if appendix.is_none() {
        if let Some(m) = LEGACY_RE.find(body) {
            body = &body[..m.start()];
        }
    }
    body
}

static MAIN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<main)(?-u:\b)[^>]*>((?s:.)*?)(?i-u:</main>)").unwrap());
static DETAILS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<details)(?-u:\b)(?s:.)*?(?i-u:</details>)").unwrap());
static TABLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i-u:<table)(?-u:\b)(?s:.)*?(?i-u:</table>)").unwrap());
static LETTER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{L}").unwrap());

/// Words in the main view: inside <main>, outside every <details> (the collapsed sections) and
/// outside tables (chart data). Labels count; bare numbers do not. The protocol's budget is about 180.
pub fn main_view_words(html: &str) -> Option<usize> {
    let inner = MAIN_RE.captures(html)?.get(1)?.as_str();
    let open = DETAILS_RE.replace_all(inner, " ");
    let open = TABLE_RE.replace_all(&open, " ");
    let text = visible_text(&open);
    if text.is_empty() {
        return Some(0);
    }
    Some(text.split(' ').filter(|w| LETTER_RE.is_match(w)).count())
}
