// Checks a finished brief, or the whole output folder, against the protocol's hard rules.
// Problems fail the check. Notes are for the owner and do not.
use crate::error::{Result, io_at};
use crate::folder::{ARCHIVE_DIR, Location, history, list_files, sorted_names};
use crate::html::{Stamp, decode_entities, main_body, read_stamp, visible_text};
use crate::naming::{Kind, META_KEY, Parsed, dashed_date, days_ts, is_brief, parse_name, today_stamp};
use crate::registry::{REGISTRY_FILE, load_registry};
use crate::util::{NOT_WS, WS, basename, is_ws, join};
use regex::{Regex, RegexBuilder};
use std::collections::HashSet;
use std::sync::LazyLock;

pub const DEFAULT_BANNED: [&str; 16] = [
    "honest",
    "honestly",
    "load-bearing",
    "earns its place",
    "genuinely",
    "quietly",
    "crucially",
    "leverage",
    "leverages",
    "leveraged",
    "seamless",
    "seamlessly",
    "delve",
    "game-changer",
    "world-class",
    "not just",
];

/// A problem list that keeps the first occurrence of each problem, in order.
#[derive(Default)]
struct Found {
    list: Vec<String>,
    seen: HashSet<String>,
}

impl Found {
    fn add(&mut self, s: impl Into<String>) {
        let s = s.into();
        if self.seen.insert(s.clone()) {
            self.list.push(s);
        }
    }
}

// ---------------------------------------------------------------------------------------
// Identifiers that do not belong above the appendices.

struct Identifier {
    what: &'static str,
    re: Regex,
    accept: fn(&str, usize, &str) -> bool,
}

fn any(_: &str, _: usize, _: &str) -> bool {
    true
}

/// A commit hash has at least one letter and one digit.
fn hash_like(_: &str, _: usize, m: &str) -> bool {
    m.bytes().any(|b| (b'a'..=b'f').contains(&b)) && m.bytes().any(|b| b.is_ascii_digit())
}

/// "#12" counts unless a word character or "&" comes right before the "#".
fn number_like(text: &str, start: usize, _: &str) -> bool {
    !text[..start].chars().next_back().is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '&')
}

static IDENTIFIERS: LazyLock<Vec<Identifier>> = LazyLock::new(|| {
    let id = |what, re: &str, accept| Identifier { what, re: Regex::new(re).unwrap(), accept };
    vec![
        id("ticket or task id", r"(?-u:\b)[A-Z][A-Z0-9]+-(?:[0-9]+|[A-Z0-9][A-Z0-9-]{2,})(?-u:\b)", any),
        id("commit hash", r"(?-u:\b)[0-9a-f]{7,40}(?-u:\b)", hash_like),
        id("pull request or issue number", r"#[0-9]{1,5}(?-u:\b)", number_like),
        id("file path", r"(?-u:\b)[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]*\.(?:md|json|ts|tsx|js|mjs|cjs|py|sh|ya?ml|toml|rs|go|html|css)(?-u:\b)", any),
        id("version string", r"(?-u:\b)v?[0-9]+\.[0-9]+\.[0-9]+(?-u:\b)", any),
    ]
});

/// The stamp inside a page must agree with its file name: kind, project and date.
pub fn stamp_mismatch(stamp: &Stamp, parsed: Option<&Parsed>) -> Vec<String> {
    let (Some(parsed), Some(template)) = (parsed, stamp.template.as_deref().filter(|t| !t.is_empty())) else {
        return vec![];
    };
    let mut out = Vec::new();
    let kind = template.split(' ').next().unwrap_or("");
    if kind != parsed.kind.as_str() {
        out.push(format!("is a {kind} brief but named as a {} brief", parsed.kind.as_str()));
    }
    if parsed.kind == Kind::Project && stamp.repo.as_deref() != parsed.repo.as_deref() {
        out.push(format!(
            "is stamped for \"{}\" but named for \"{}\"",
            stamp.repo.as_deref().unwrap_or("null"),
            parsed.repo.as_deref().unwrap_or("")
        ));
    }
    let named = dashed_date(&parsed.date);
    if let Some(date) = stamp.date.as_deref().filter(|d| !d.is_empty()) {
        if date != named {
            out.push(format!("is stamped {date} but named {named}"));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------
// Markup allowlist. A brief may contain only the markup brf's templates produce: these tags,
// these attributes, links that need a tap, radio buttons and their labels (for a theme switch
// without script), and CSS whose only url() targets are embedded data. Anything else fails, however
// it is written. The reader below follows the HTML rules that matter for that: comments (including
// "<!-->" and "--!>"), bogus comments ("<!x", "<?x", "</3"), text-only elements such as <style>
// and <title>, SVG content, and HTML whitespace.

fn is_html_ws(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0C | b'\r' | b' ')
}

const RAW_TEXT: [&str; 10] = ["script", "style", "xmp", "iframe", "noembed", "noframes", "noscript", "textarea", "title", "plaintext"];

#[derive(Clone, Debug)]
pub struct Attr {
    pub name: String,
    pub raw: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub attrs: Vec<Attr>,
    /// Inside <svg> or <math>.
    pub foreign: bool,
}

struct ReadTag {
    name: String,
    attrs: Vec<Attr>,
    self_closing: bool,
    end: usize,
}

fn find_byte(b: &[u8], needle: u8, from: usize) -> Option<usize> {
    b.get(from..)?.iter().position(|&c| c == needle).map(|i| i + from)
}

fn find_str(b: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if from > b.len() {
        return None;
    }
    b[from..].windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

fn read_tag(html: &str, start: usize) -> ReadTag {
    let b = html.as_bytes();
    let n = b.len();
    let mut j = start;
    while j < n && !is_html_ws(b[j]) && b[j] != b'/' && b[j] != b'>' {
        j += 1;
    }
    let name = html[start..j].to_lowercase();
    let mut attrs = Vec::new();
    let mut self_closing = false;
    loop {
        while j < n && (is_html_ws(b[j]) || b[j] == b'/') {
            if b[j] == b'/' && b.get(j + 1) == Some(&b'>') {
                self_closing = true;
            }
            j += 1;
        }
        if j >= n {
            break;
        }
        if b[j] == b'>' {
            j += 1;
            break;
        }
        // The first character of a name may be anything, even "=" or a quote.
        let mut k = j + 1;
        while k < n && !is_html_ws(b[k]) && b[k] != b'/' && b[k] != b'>' && b[k] != b'=' {
            k += 1;
        }
        let k = k.min(n);
        let attr_name = String::from_utf8_lossy(&b[j..k]).to_lowercase();
        j = k;
        while j < n && is_html_ws(b[j]) {
            j += 1;
        }
        let mut raw: Option<String> = None;
        if j < n && b[j] == b'=' {
            j += 1;
            while j < n && is_html_ws(b[j]) {
                j += 1;
            }
            let q = b.get(j).copied();
            if q == Some(b'"') || q == Some(b'\'') {
                let close = find_byte(b, q.unwrap(), j + 1);
                raw = Some(html[j + 1..close.unwrap_or(n)].to_string());
                j = close.map_or(n, |c| c + 1);
            } else {
                let mut e = j;
                while e < n && !is_html_ws(b[e]) && b[e] != b'>' {
                    e += 1;
                }
                raw = Some(html[j..e].to_string());
                j = e;
            }
        }
        let raw = raw.unwrap_or_default();
        let value = decode_entities(&raw);
        attrs.push(Attr { name: attr_name, raw, value });
    }
    ReadTag { name, attrs, self_closing, end: j }
}

/// Where the end tag "</name" starts (ASCII case-insensitive), from `from`.
fn find_end_tag(html: &str, name: &str, from: usize) -> Option<usize> {
    let b = html.as_bytes();
    let needle = format!("</{name}");
    let nb = needle.as_bytes();
    let mut at = from;
    loop {
        let i = b.get(at..)?.windows(nb.len()).position(|w| w.eq_ignore_ascii_case(nb))? + at;
        match b.get(i + nb.len()) {
            None => return Some(i),
            Some(&c) if is_html_ws(c) || c == b'/' || c == b'>' => return Some(i),
            _ => at = i + 2,
        }
    }
}

/// Start tags (with whether they sit inside SVG or MathML) and the text of each <style> element.
pub fn scan_markup(html: &str) -> (Vec<Tag>, Vec<String>) {
    let b = html.as_bytes();
    let n = b.len();
    let mut tags = Vec::new();
    let mut styles = Vec::new();
    let mut i = 0;
    let mut foreign = 0usize;
    while i < n {
        let Some(lt) = find_byte(b, b'<', i) else { break };
        let c1 = b.get(lt + 1).copied();
        if b[lt..].starts_with(b"<!--") {
            let j = lt + 4;
            if b.get(j) == Some(&b'>') {
                i = j + 1;
                continue;
            }
            if b.get(j..).is_some_and(|r| r.starts_with(b"->")) {
                i = j + 2;
                continue;
            }
            let a = find_str(b, b"-->", j).map(|x| x + 3);
            let c = find_str(b, b"--!>", j).map(|x| x + 4);
            i = [a, c].into_iter().flatten().min().unwrap_or(n);
            continue;
        }
        if c1 == Some(b'!') || c1 == Some(b'?') {
            i = find_byte(b, b'>', lt + 2).map_or(n, |e| e + 1);
            continue;
        }
        if c1 == Some(b'/') {
            if b.get(lt + 2).is_some_and(u8::is_ascii_alphabetic) {
                let t = read_tag(html, lt + 2);
                if (t.name == "svg" || t.name == "math") && foreign > 0 {
                    foreign -= 1;
                }
                i = t.end;
            } else {
                i = find_byte(b, b'>', lt + 2).map_or(n, |e| e + 1);
            }
            continue;
        }
        if c1.is_some_and(|c| c.is_ascii_alphabetic()) {
            let t = read_tag(html, lt + 1);
            i = t.end;
            let is_foreign_root = t.name == "svg" || t.name == "math";
            let raw_text = foreign == 0 && RAW_TEXT.contains(&t.name.as_str());
            let name = t.name.clone();
            tags.push(Tag { name: t.name, attrs: t.attrs, foreign: foreign > 0 });
            if is_foreign_root {
                if !t.self_closing {
                    foreign += 1;
                }
                continue;
            }
            if raw_text {
                if name == "plaintext" {
                    i = n;
                    continue;
                }
                let close = find_end_tag(html, &name, i);
                if name == "style" {
                    styles.push(html[i..close.unwrap_or(n)].to_string());
                }
                i = close.unwrap_or(n);
            }
            continue;
        }
        i = lt + 1;
    }
    (tags, styles)
}

const HTML_TAGS: &[&str] = &[
    "html",
    "head",
    "body",
    "meta",
    "title",
    "style",
    "header",
    "footer",
    "main",
    "section",
    "article",
    "aside",
    "nav",
    "div",
    "span",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "ul",
    "ol",
    "li",
    "dl",
    "dt",
    "dd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "caption",
    "colgroup",
    "col",
    "strong",
    "em",
    "b",
    "i",
    "small",
    "sub",
    "sup",
    "abbr",
    "time",
    "code",
    "kbd",
    "pre",
    "blockquote",
    "mark",
    "s",
    "u",
    "br",
    "wbr",
    "hr",
    "figure",
    "figcaption",
    "details",
    "summary",
    "a",
    "svg",
    "input",
    "label",
];

fn html_tag_allowed(name: &str) -> bool {
    HTML_TAGS.contains(&name)
}

/// A radio button and its label: the only controls a brief may carry, for a theme switch that needs
/// no script. Nothing else on them: no value, src, form or formaction, no style, no event handler.
/// A radio cannot fetch or run anything; every other input type is refused.
fn control_attrs(tag: &str) -> Option<&'static [&'static str]> {
    match tag {
        "input" => Some(&["type", "name", "id", "checked", "class", "aria-label"]),
        "label" => Some(&["for", "class", "title", "aria-label"]),
        _ => None,
    }
}

const SVG_TAGS: [&str; 18] = [
    "svg",
    "g",
    "path",
    "circle",
    "ellipse",
    "rect",
    "line",
    "polyline",
    "polygon",
    "title",
    "desc",
    "defs",
    "lineargradient",
    "radialgradient",
    "stop",
    "clippath",
    "text",
    "tspan",
];
const GLOBAL_ATTRS: [&str; 9] = ["class", "id", "lang", "dir", "title", "role", "hidden", "style", "translate"];

fn tag_attrs(tag: &str) -> &'static [&'static str] {
    match tag {
        "html" => &["data-style"],
        "meta" => &["charset", "name", "content"],
        "a" => &["href", "rel", "target"],
        "th" => &["scope", "colspan", "rowspan"],
        "td" => &["colspan", "rowspan"],
        "time" => &["datetime"],
        "details" => &["open"],
        "ol" => &["start", "reversed"],
        "col" | "colgroup" => &["span"],
        _ => &[],
    }
}

const SVG_ATTRS: &[&str] = &[
    "xmlns",
    "xmlns:xlink",
    "viewbox",
    "width",
    "height",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-opacity",
    "fill-opacity",
    "fill-rule",
    "clip-rule",
    "clip-path",
    "d",
    "x",
    "y",
    "x1",
    "y1",
    "x2",
    "y2",
    "cx",
    "cy",
    "r",
    "rx",
    "ry",
    "fx",
    "fy",
    "dx",
    "dy",
    "points",
    "transform",
    "opacity",
    "offset",
    "stop-color",
    "stop-opacity",
    "gradientunits",
    "gradienttransform",
    "spreadmethod",
    "clippathunits",
    "font-family",
    "font-size",
    "font-weight",
    "text-anchor",
    "dominant-baseline",
    "letter-spacing",
    "preserveaspectratio",
    "version",
    "focusable",
    "shape-rendering",
    "vector-effect",
    "paint-order",
    "xml:space",
    "display",
    "visibility",
    "color",
];

fn svg_attr_allowed(name: &str) -> bool {
    SVG_ATTRS.contains(&name)
}

static ARIA_DATA_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:aria|data)-[a-z0-9-]+$").unwrap());
static NAMED_ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&[a-zA-Z][a-zA-Z0-9]*;").unwrap());
static STYLE_ATTR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9\t\n\x0C\r :;.%#,-]*$").unwrap());
static LINK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^(?:(?i-u:https?)://{NOT_WS}|(?i-u:mailto):{NOT_WS}|#)")).unwrap());
static HREF_ENTITY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&(?:#|(?i-u:[a-z])+;)").unwrap());
static URL_CALL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i-u:url){WS}*\(")).unwrap());
static URL_FRAGMENT_ONLY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\t\n ]*(?i-u:url)\([\t\n ]*#[A-Za-z0-9_-]+[\t\n ]*\)[\t\n ]*$").unwrap());
static META_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"(?i-u:url){WS}*=")).unwrap());
static CSS_URL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i-u:url)\([\t\n ]*(["']?)[\t\n ]*"#).unwrap());

/// CSS line endings: CR LF, CR and form feed each become one newline.
fn css_newlines(css: &str) -> String {
    css.replace("\r\n", "\n").replace(['\r', '\x0C'], "\n")
}

/// Walk the CSS escapes in `css`: hex escapes (1 to 6 digits) and single escaped characters.
/// `on_escape` gets the escaped code point and returns what to write for it; with
/// `eat_ws`, one tab, newline or space after a hex escape belongs to the escape.
fn walk_escapes(css: &str, eat_ws: bool, mut on_escape: impl FnMut(u32, &mut String)) -> String {
    let mut out = String::with_capacity(css.len());
    let mut it = css.char_indices().peekable();
    while let Some((_, c)) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let mut hex = String::new();
        while hex.len() < 6 {
            match it.peek() {
                Some(&(_, h)) if h.is_ascii_hexdigit() => {
                    hex.push(h);
                    it.next();
                }
                _ => break,
            }
        }
        if !hex.is_empty() {
            if eat_ws {
                if let Some(&(_, w)) = it.peek() {
                    if w == '\t' || w == '\n' || w == ' ' {
                        it.next();
                    }
                }
            }
            on_escape(u32::from_str_radix(&hex, 16).unwrap_or(0), &mut out);
        } else if let Some((_, ch)) = it.next() {
            on_escape(ch as u32 | 0x8000_0000, &mut out);
        } else {
            out.push('\\');
        }
    }
    out
}

/// CSS escapes ("\72" is "r") decoded, so "u\72l(" is seen as "url(". Line endings are
/// normalised first, as CSS does, so CR LF after an escape counts as one newline. One pass,
/// like the CSS tokenizer: an escaped backslash stays a backslash.
pub fn decode_css_escapes(css: &str) -> String {
    walk_escapes(&css_newlines(css), true, |code, out| {
        if code & 0x8000_0000 != 0 {
            out.push(char::from_u32(code & 0x7FFF_FFFF).unwrap_or('\u{FFFD}'));
        } else {
            match char::from_u32(code) {
                Some(ch) if code > 0 => out.push(ch),
                _ => out.push('\u{FFFD}'),
            }
        }
    })
}

fn contains_ci(hay: &str, needle: &str) -> bool {
    hay.as_bytes().windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle.as_bytes()))
}

fn css_problems(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Comments are not removed first: a comment marker inside a CSS string would hide what
    // follows. Looking at everything can only flag more, never less.
    let decoded = decode_css_escapes(css);
    // An escaped quote or backslash can change where a url() argument starts. brf's CSS has none.
    let mut quote = false;
    walk_escapes(&css_newlines(css), false, |code, _| {
        let value = code & 0x7FFF_FFFF;
        if value == 0x22 || value == 0x27 || value == 0x5c {
            quote = true;
        }
    });
    if quote {
        out.push("its CSS escapes a quote or backslash".to_string());
    }
    if contains_ci(&decoded, "@import") {
        out.push("its CSS imports another stylesheet".to_string());
    }
    if contains_ci(&decoded, "image-set(") {
        out.push("its CSS uses image-set()".to_string());
    }
    // CSS whitespace only (after normalising, tab, newline and space): a no-break space is part
    // of the url, not padding around it.
    for m in CSS_URL_RE.find_iter(&decoded) {
        let rest = &decoded.as_bytes()[m.end()..];
        if !(rest.len() >= 5 && rest[..5].eq_ignore_ascii_case(b"data:")) {
            out.push("its CSS fetches from the network (a url() that is not embedded data)".to_string());
            break;
        }
    }
    out
}

/// Everything in a page that is outside brf's markup allowlist.
pub fn markup_problems(html: &str) -> Vec<String> {
    let (tags, styles) = scan_markup(html);
    allowlist_problems(&tags, &styles)
}

/// The allowlist itself, applied to a list of start tags and <style> texts.
pub fn allowlist_problems(tags: &[Tag], styles: &[String]) -> Vec<String> {
    let mut found = Found::default();
    for tag in tags {
        let allowed = if tag.foreign { SVG_TAGS.contains(&tag.name.as_str()) } else { html_tag_allowed(&tag.name) };
        if !allowed {
            found.add(if tag.name == "script" {
                "has a script element".to_string()
            } else {
                format!("uses a <{}> element, which brf does not allow", tag.name)
            });
            continue;
        }
        let svgish = tag.foreign || tag.name == "svg";
        let control = if tag.foreign { None } else { control_attrs(&tag.name) };
        if control.is_some() && tag.name == "input" {
            let types: Vec<&Attr> = tag.attrs.iter().filter(|a| a.name == "type").collect();
            if types.len() != 1 || types[0].value.to_lowercase() != "radio" {
                found.add("has an <input> that is not type=\"radio\", which brf does not allow");
            }
        }
        for Attr { name, raw, value } in &tag.attrs {
            if name.starts_with("on") {
                found.add("has an event-handler attribute");
                continue;
            }
            if let Some(control) = control {
                if !control.contains(&name.as_str()) {
                    found.add(format!("has a \"{name}\" attribute on <{}>, which brf does not allow", tag.name));
                    continue;
                }
                if NAMED_ENTITY_RE.is_match(value) {
                    found.add(format!("has an entity brf cannot read in the \"{name}\" attribute"));
                }
                continue;
            }
            let ok = ARIA_DATA_RE.is_match(name)
                || GLOBAL_ATTRS.contains(&name.as_str())
                || tag_attrs(&tag.name).contains(&name.as_str())
                || (svgish && svg_attr_allowed(name));
            if !ok {
                found.add(format!("has a \"{name}\" attribute on <{}>, which brf does not allow", tag.name));
                continue;
            }
            // An entity brf cannot decode could spell anything, such as "(" in "url&lpar;".
            if NAMED_ENTITY_RE.is_match(value) {
                found.add(format!("has an entity brf cannot read in the \"{name}\" attribute"));
                continue;
            }
            if name == "style" && !(STYLE_ATTR_RE.is_match(raw) && STYLE_ATTR_RE.is_match(value)) {
                found.add("has a style attribute brf does not allow (only plain values like width:40%)");
            }
            // SVG presentation attributes are CSS values, so CSS escapes apply to them too.
            let css_value = decode_css_escapes(value);
            if URL_CALL_RE.is_match(&css_value) && !URL_FRAGMENT_ONLY_RE.is_match(&css_value) {
                found.add(format!("has a url() in the \"{name}\" attribute"));
            }
            if name == "href" {
                let compact: String = value.chars().filter(|&c| !(is_ws(c) || (c as u32) < 0x20)).collect();
                if !LINK_RE.is_match(&compact) || HREF_ENTITY_RE.is_match(value) {
                    found.add("has a link that is not https, http, mailto or a # anchor");
                }
            }
            if tag.name == "meta" && name == "content" && META_URL_RE.is_match(value) {
                found.add("has a meta tag that points somewhere");
            }
        }
    }
    for css in styles {
        for p in css_problems(css) {
            found.add(p);
        }
    }
    found.list
}

fn banned_re(word: &str) -> Option<Regex> {
    let body = regex::escape(word);
    let pattern = if word.is_ascii() { format!(r"(?-u:\b)(?i-u:{body})(?-u:\b)") } else { format!(r"(?-u:\b)(?i:{body})(?-u:\b)") };
    RegexBuilder::new(&pattern).build().ok()
}

pub fn check_html(html: &str, name: &str, banned_extra: &[String]) -> Vec<String> {
    let mut problems = markup_problems(html);
    let parsed = parse_name(name);
    let stamp = read_stamp(html);
    let filled = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.is_empty());
    if !filled(&stamp.brf_version) || !filled(&stamp.template) || stamp.brief.is_none_or(|b| b == 0) {
        problems.push("has no brf version stamp (build it with brf build)".to_string());
    }
    problems.extend(stamp_mismatch(&stamp, parsed.as_ref()));
    let text = visible_text(html);
    let dashes = text.matches('\u{2014}').count();
    if dashes > 0 {
        problems.push(format!("has {dashes} em-dash(es)"));
    }
    let banned: Vec<String> = DEFAULT_BANNED
        .iter()
        .map(|s| s.to_string())
        .chain(banned_extra.iter().cloned())
        .filter(|w| banned_re(w).is_some_and(|re| re.is_match(&text)))
        .collect();
    if !banned.is_empty() {
        problems.push(format!("uses banned words: {}", banned.join(", ")));
    }
    let body = visible_text(main_body(html));
    for Identifier { what, re, accept } in IDENTIFIERS.iter() {
        let mut hits: Vec<&str> = Vec::new();
        for m in re.find_iter(&body) {
            if accept(&body, m.start(), m.as_str()) && !hits.contains(&m.as_str()) {
                hits.push(m.as_str());
            }
        }
        if !hits.is_empty() {
            problems.push(format!("shows a {what} above the appendices: {}", hits.iter().take(6).copied().collect::<Vec<_>>().join(", ")));
        }
    }
    problems
}

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
