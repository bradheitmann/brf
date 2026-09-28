// Template versions and where the templates live.
//
// Each template declares its own version in a meta tag:
//   <meta name="brf-template" content="project 1.0.0">
// The build reads it from the filled file and stamps it into the finished page.
//
// The built-in templates ship inside the binary. When the binary runs from a brf checkout
// (target/release/brf, or a test binary), the checkout's own files are used; otherwise the
// embedded copies are written once to <cache>/assets/<version>/ so agents have a path to copy.
use crate::config::{Env, cache_dir};
use crate::error::{Result, io_at};
use crate::util::{WS, join};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, OnceLock};

pub const TEMPLATE_FILES: [(&str, &str); 2] = [("project", "project.html"), ("meta", "meta.html")];

const EMBEDDED: [(&str, &str); 5] = [
    ("templates/project.html", include_str!("../templates/project.html")),
    ("templates/meta.html", include_str!("../templates/meta.html")),
    ("templates/fonts.css", include_str!("../templates/fonts.css")),
    ("verify/verify.ts", include_str!("../verify/verify.ts")),
    ("verify/package.json", "{\n  \"private\": true,\n  \"type\": \"module\"\n}\n"),
];

pub fn brf_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn is_checkout(dir: &Path) -> bool {
    ["templates/project.html", "templates/meta.html", "templates/fonts.css", "verify/verify.ts", "SKILL.md"].iter().all(|f| dir.join(f).is_file())
}

fn checkout_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    exe.ancestors().skip(1).take(4).find(|d| is_checkout(d)).map(Path::to_path_buf)
}

fn materialize() -> PathBuf {
    let dir = PathBuf::from(join(&[&cache_dir(&Env::process()), "assets", brf_version()]));
    for (rel, text) in EMBEDDED {
        let path = dir.join(rel);
        if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
            continue;
        }
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
    dir
}

/// brf's own folder: the checkout the binary was built in, or the unpacked built-in files.
pub fn root() -> &'static str {
    static ROOT: OnceLock<String> = OnceLock::new();
    ROOT.get_or_init(|| checkout_root().unwrap_or_else(materialize).to_string_lossy().into_owned())
}

pub fn builtin_templates_dir() -> String {
    join(&[root(), "templates"])
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateTag {
    pub kind: String,
    pub version: String,
    pub major: f64,
    pub minor: f64,
    pub patch: f64,
}

static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"<meta{WS}+name="brf-template"{WS}+content="(project|meta){WS}+([0-9]+)\.([0-9]+)\.([0-9]+)"{WS}*/?>"#)).unwrap()
});

fn num(s: &str) -> f64 {
    s.parse::<f64>().unwrap_or(f64::NAN)
}

pub fn read_template_tag(html: &str) -> Option<TemplateTag> {
    let c = TAG_RE.captures(html)?;
    Some(TemplateTag {
        kind: c[1].to_string(),
        version: format!("{}.{}.{}", &c[2], &c[3], &c[4]),
        major: num(&c[2]),
        minor: num(&c[3]),
        patch: num(&c[4]),
    })
}

/// The owner may keep their own templates (their brand, their fonts) outside this repository:
/// BRF_TEMPLATES_DIR or the config file's templates_dir. A kind the folder does not provide, and
/// fonts.css when it is absent, come from the built-in templates.
pub fn templates_dir(env: &Env) -> Option<String> {
    let mut dir = env.get("BRF_TEMPLATES_DIR").map(str::to_string);
    if dir.is_none() {
        let home = env.get("HOME").unwrap_or("").to_string();
        let cfg_path = match env.get("BRF_CONFIG") {
            Some(p) => p.to_string(),
            None => {
                let base = env.get("XDG_CONFIG_HOME").map(str::to_string).unwrap_or_else(|| join(&[&home, ".config"]));
                join(&[&base, "brf", "config.json"])
            }
        };
        if Path::new(&cfg_path).exists() {
            dir = std::fs::read_to_string(&cfg_path)
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .and_then(|v| v.get("templates_dir").and_then(|d| d.as_str()).filter(|d| !d.is_empty()).map(str::to_string));
            if let Some(d) = &dir {
                if let Some(rest) = d.strip_prefix("~/") {
                    dir = Some(join(&[&home, rest]));
                }
            }
        }
    }
    dir.filter(|d| Path::new(d).exists())
}

pub fn template_path(file: &str, env: &Env) -> String {
    if let Some(own) = templates_dir(env) {
        let p = join(&[&own, file]);
        if Path::new(&p).exists() {
            return p;
        }
    }
    join(&[&builtin_templates_dir(), file])
}

#[derive(Clone, Debug)]
pub struct Current {
    pub kind: &'static str,
    pub path: String,
    pub version: Option<String>,
    pub major: Option<f64>,
}

/// The template of each kind this computer builds from, in the order project, meta.
pub fn current_templates(env: &Env) -> Result<Vec<Current>> {
    let mut out = Vec::new();
    for (kind, file) in TEMPLATE_FILES {
        let path = template_path(file, env);
        let text = std::fs::read_to_string(&path).map_err(|e| io_at(e, "open", &path))?;
        let tag = read_template_tag(&text);
        out.push(Current { kind, version: tag.as_ref().map(|t| t.version.clone()), major: tag.map(|t| t.major), path });
    }
    Ok(out)
}

pub fn current_template(env: &Env, kind: &str) -> Result<Current> {
    Ok(current_templates(env)?.into_iter().find(|c| c.kind == kind).expect("known template kind"))
}

pub fn compare_versions(a: &str, b: &str) -> i32 {
    let pa: Vec<f64> = a.split('.').map(num).collect();
    let pb: Vec<f64> = b.split('.').map(num).collect();
    for i in 0..3 {
        let x = pa.get(i).copied().unwrap_or(f64::NAN);
        let y = pb.get(i).copied().unwrap_or(f64::NAN);
        if x != y {
            return if x < y { -1 } else { 1 };
        }
    }
    0
}
