// Turn a filled template into the finished, self-contained page.
//
// In order: read the template tag, check the output name, work out the brief number,
// embed the fonts, write the brief number into {{BRIEF_NUMBER}}, refuse anything left unfilled,
// strip every comment, stamp the versions, write the file.
use crate::check::markup_problems;
use crate::config::Env;
use crate::error::{Error, Result, io_at};
use crate::folder::next_number;
use crate::html::{main_view_words, read_meta};
use crate::naming::{Kind, dashed_date, parse_name};
use crate::registry::{find_project, load_registry};
use crate::templates::{brf_version, compare_versions, current_template, read_template_tag, template_path};
use crate::util::{WS, basename, dirname, number_string, resolve, thousands, trim, utf16_len};
use regex::{NoExpand, Regex};
use std::path::Path;
use std::sync::LazyLock;

const FONTS_MARKER: &str = "/* {{FONTS_CSS}} */";

pub fn colophon_text(kind: Kind, brief: u64, brf: &str, template: &str) -> String {
    let what = match kind {
        Kind::Meta => format!("Meta brief {brief}"),
        Kind::Project => format!("Brief {brief} of this project"),
    };
    format!("{what} · brf {brf} · {} template {template}", kind.as_str())
}

#[derive(Clone, Debug)]
pub struct BuildOptions<'a> {
    pub input: &'a str,
    pub out: &'a str,
    /// A brief number to use instead of the next one (--brief). NaN and fractions are refused.
    pub brief: Option<f64>,
    pub example: bool,
    pub output_dir: Option<&'a str>,
    pub env: &'a Env,
}

#[derive(Clone, Debug)]
pub struct Built {
    pub path: String,
    pub name: String,
    pub kind: Kind,
    pub repo: Option<String>,
    pub date: String,
    pub brief: u64,
    pub brf_version: String,
    pub template: String,
}

static SLOT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{\{([A-Z0-9_]*)\}\}").unwrap());
static REPEAT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"<!--{WS}*/?repeat(?-u:\b)[^>]*-->")).unwrap());
static COMMENT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!--(?s:.)*?-->\n?").unwrap());
static TAG_META_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r#"(<meta{WS}+name="brf-template"[^>]*>)"#)).unwrap());

pub fn build(opts: &BuildOptions<'_>, log: &mut dyn FnMut(&str)) -> Result<Built> {
    let src = resolve(&[opts.input]);
    if !Path::new(&src).exists() {
        return Err(Error::brf("NOT_FOUND", format!("Not found: {src}")));
    }
    if opts.out.is_empty() {
        return Err(Error::brf("NO_OUT", "Pass --out with a path in your work folder, e.g. --out <work_dir>/brf_my-project_20260131.html"));
    }
    let dest = resolve(&[opts.out]);
    if dest == src {
        return Err(Error::brf("SAME_FILE", "The output would overwrite the input."));
    }

    let mut html = String::from_utf8_lossy(&std::fs::read(&src).map_err(|e| io_at(e, "open", &src))?).into_owned();
    if read_meta(&html, "brf-version").is_some_and(|v| !v.is_empty()) {
        return Err(Error::brf("ALREADY_BUILT", format!("{} is already a finished brief. Build from the filled template.", basename(&src))));
    }

    let Some(tag) = read_template_tag(&html) else {
        return Err(Error::brf(
            "NO_TEMPLATE_TAG",
            "The filled file has no <meta name=\"brf-template\"> tag. Start from a template in templates/ and keep that tag.",
        ));
    };
    let current = current_template(opts.env, &tag.kind)?;
    if current.major != Some(tag.major) {
        return Err(Error::brf(
            "TEMPLATE_MAJOR",
            format!(
                "The filled file uses {} template {}; this brf ships {}. Refill from templates/{}.",
                tag.kind,
                tag.version,
                current.version.as_deref().unwrap_or("null"),
                basename(&current.path)
            ),
        ));
    }
    let current_version = current.version.clone().unwrap_or_default();
    if compare_versions(&tag.version, &current_version) < 0 {
        log(&format!("note      {} template {} is older than {current_version}; it still builds", tag.kind, tag.version));
    }

    let dest_name = basename(&dest);
    let parsed = match parse_name(&dest_name) {
        Some(p) if p.ext == "html" && p.variant.is_none() && !p.has_v() => p,
        _ => {
            return Err(Error::brf(
                "BAD_NAME",
                format!(
                    "\"{dest_name}\" is not a brief name. Use brf_<repo-name>_<yyyymmdd>.html or meta_brf_<yyyymmdd>.html, e.g. <work_dir>/brf_my-project_20260131.html"
                ),
            ));
        }
    };
    if parsed.kind.as_str() != tag.kind {
        return Err(Error::brf(
            "KIND_MISMATCH",
            format!("A {} template cannot become {dest_name}. Project briefs are brf_…, the meta brief is meta_brf_….", tag.kind),
        ));
    }

    // Which project, and which brief number.
    if !opts.example {
        let Some(out_dir) = opts.output_dir else {
            return Err(Error::brf(
                "NO_OUTPUT_DIR",
                "No output folder is set, so the brief number and registry cannot be checked. Run brf init, or pass --example for a sample page.",
            ));
        };
        if parsed.kind == Kind::Project {
            let repo = parsed.repo.as_deref().unwrap_or("");
            if find_project(&load_registry(out_dir)?, repo).is_none() {
                return Err(Error::brf(
                    "NOT_REGISTERED",
                    format!("\"{repo}\" is not in the registry. Briefs are made only for projects the owner has asked for (brf register)."),
                ));
            }
        }
    }
    let number: f64 = match opts.brief {
        Some(n) => n,
        None if opts.example => 1.0,
        None => next_number(opts.output_dir.unwrap_or(""), &parsed.key) as f64,
    };
    if !(number.is_finite() && number.fract() == 0.0 && number >= 1.0) {
        return Err(Error::brf("BAD_BRIEF_NUMBER", format!("The brief number must be a whole number from 1; got {}.", number_string(number))));
    }
    let number = number as u64;

    // Fonts.
    let fonts_path = template_path("fonts.css", opts.env);
    if !html.contains(FONTS_MARKER) {
        return Err(Error::brf("NO_FONTS_MARKER", format!("The fonts marker is gone. Put {FONTS_MARKER} back inside the style element.")));
    }
    let fonts_css = String::from_utf8_lossy(&std::fs::read(&fonts_path).map_err(|e| io_at(e, "open", &fonts_path))?).into_owned();
    html = html.replacen(FONTS_MARKER, &fonts_css, 1);
    log(&format!("fonts     embedded {} bytes", thousands(utf16_len(&fonts_css))));

    // The brief number is brf's to write, so the page and its stamp always agree.
    html = html.replace("{{BRIEF_NUMBER}}", &number.to_string());

    // Nothing left unfilled.
    let mut problems: Vec<String> = SLOT_RE.captures_iter(&html).map(|c| format!("unfilled slot  {{{{{}}}}}", &c[1])).collect();
    problems.extend(REPEAT_RE.find_iter(&html).map(|m| format!("repeat marker  {}", trim(m.as_str()))));
    if !problems.is_empty() {
        let mut counts: Vec<(String, usize)> = Vec::new();
        for p in &problems {
            match counts.iter_mut().find(|(q, _)| q == p) {
                Some((_, n)) => *n += 1,
                None => counts.push((p.clone(), 1)),
            }
        }
        let list: Vec<String> = counts.iter().map(|(p, n)| format!("  {p}{}", if *n > 1 { format!("  ({n}x)") } else { String::new() })).collect();
        return Err(Error::brf(
            "UNFILLED",
            format!(
                "Refused. {} thing(s) still to do in {}:\n{}\nFill every slot, expand each repeat block and delete its two markers, then build again.",
                problems.len(),
                basename(&src),
                list.join("\n")
            ),
        ));
    }

    // Comments carry guidance only; none ships.
    let before = utf16_len(&html);
    html = COMMENT_RE.replace_all(&html, "").into_owned();
    log(&format!("comments  stripped {} bytes of guidance", thousands(before - utf16_len(&html))));

    // Only brf's markup allowlist may ship: nothing that runs code or reaches the network.
    let markup = markup_problems(&html);
    if !markup.is_empty() {
        return Err(Error::brf("UNSAFE_MARKUP", format!("Refused. The page {}.", markup.join("; "))));
    }

    // Stamp.
    let brf = brf_version();
    let mut stamp_meta = vec![format!("<meta name=\"brf-version\" content=\"{brf}\">"), format!("<meta name=\"brf-brief\" content=\"{number}\">")];
    if let (Kind::Project, Some(repo)) = (parsed.kind, &parsed.repo) {
        stamp_meta.push(format!("<meta name=\"brf-repo\" content=\"{repo}\">"));
    }
    stamp_meta.push(format!("<meta name=\"brf-date\" content=\"{}\">", dashed_date(&parsed.date)));
    let stamp_meta = stamp_meta.join("\n");
    if let Some(m) = TAG_META_RE.find(&html) {
        let end = m.end();
        html = format!("{}\n{stamp_meta}{}", &html[..end], &html[end..]);
    }
    let colophon_line = colophon_text(parsed.kind, number, brf, &tag.version);
    let colophon = format!("<span data-brf=\"colophon\" style=\"display:block;margin-top:8px\">{colophon_line}</span>");
    if let Some(at) = html.rfind("</footer>") {
        html = format!("{}{colophon}{}", &html[..at], &html[at..]);
    } else {
        let main_end = Regex::new("</main>").expect("literal");
        html = main_end.replace(&html, NoExpand(&format!("<footer class=\"footer small\">{colophon}</footer></main>"))).into_owned();
    }

    if let Some(words) = main_view_words(&html) {
        if tag.major >= 2.0 {
            log(&format!("words     {words} in the main view, outside the collapsed sections (budget about 180)"));
        }
    }

    std::fs::create_dir_all(dirname(&dest))?;
    std::fs::write(&dest, &html).map_err(|e| io_at(e, "open", &dest))?;
    let bytes = std::fs::metadata(&dest)?.len();
    log(&format!("built     {dest}"));
    log(&format!("stamped   {colophon_line}"));
    // Kilobytes rounded half up.
    log(&format!("size      {} KB", (bytes + 512) / 1024));
    Ok(Built {
        name: dest_name,
        kind: parsed.kind,
        repo: parsed.repo.clone(),
        date: parsed.date.clone(),
        brief: number,
        brf_version: brf.to_string(),
        template: format!("{} {}", tag.kind, tag.version),
        path: dest,
    })
}
