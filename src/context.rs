// Everything an agent needs to start a brief, worked out once so it never guesses:
// which project this is, whether the owner registered it, the file name, the brief
// number, the earlier briefs to read, where to keep working files, and what tools exist.
use crate::config::{Config, Env, require_output_dir};
use crate::error::Result;
use crate::folder::{current_files, history, next_number};
use crate::naming::{Kind, META_KEY, format_name, human_date};
use crate::registry::{Checkout, Registry, load_registry, match_checkout};
use crate::repo::repo_info;
use crate::templates::{brf_version, current_templates};
use crate::util::{WS, dirname, executable, join, resolve};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

/// The first line of a tool's version output, or None when it is missing or fails.
fn tool_version(cmd: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Some(crate::util::trim(&text).split('\n').next().unwrap_or("").to_string())
}

pub fn find_on_path(name: &str, path_var: &str) -> Option<String> {
    path_var.split(':').filter(|d| !d.is_empty()).map(|d| join(&[d, name])).find(|c| executable(c))
}

pub const MISTER_CLEAN_PACKAGE: &str = "@bradheitmann/mister-clean";

static SHIM_MARK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"#{WS}*cmd-shim-target=([^\n\r\x{{2028}}\x{{2029}}]+)")).unwrap());
static SHIM_REL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"\$basedir/([^"'\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}]+\.(?:js|mjs|cjs))"#)
        .unwrap()
});

/// Package managers install small shell scripts that point at the real file. Follow them.
fn shim_target(bin_path: &str) -> Option<String> {
    if std::fs::metadata(bin_path).ok()?.len() > 65536 {
        return None;
    }
    let text = String::from_utf8_lossy(&std::fs::read(bin_path).ok()?).into_owned();
    if let Some(c) = SHIM_MARK_RE.captures(&text) {
        return Some(crate::util::trim(&c[1]).to_string());
    }
    SHIM_REL_RE.captures(&text).map(|c| resolve(&[&dirname(bin_path), &c[1]]))
}

/// The version of an installed command, read only from the package.json of the named package.
pub fn package_version_of(bin_path: &str, package_name: &str) -> Option<String> {
    let starts: Vec<String> = [Some(bin_path.to_string()), shim_target(bin_path)].into_iter().flatten().collect();
    'starts: for start in starts {
        let Ok(real) = std::fs::canonicalize(&start) else { continue };
        let mut dir = dirname(&real.to_string_lossy());
        for _ in 0..6 {
            let pkg = join(&[&dir, "package.json"]);
            if Path::new(&pkg).exists() {
                let Some(data) = std::fs::read_to_string(&pkg).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
                    continue 'starts;
                };
                if data.get("name").and_then(Value::as_str) == Some(package_name) {
                    return data.get("version").and_then(Value::as_str).filter(|v| !v.is_empty()).map(str::to_string);
                }
            }
            dir = dirname(&dir);
        }
    }
    None
}

pub fn mister_clean(path_var: &str) -> Value {
    match find_on_path("mister-clean", path_var) {
        Some(bin) => json!({ "path": bin, "version": package_version_of(&bin, MISTER_CLEAN_PACKAGE) }),
        None => Value::Null,
    }
}

fn portfolio(out_dir: &str, registry: &Registry) -> Vec<Value> {
    registry
        .projects
        .iter()
        .map(|p| {
            let cur = current_files(out_dir, &p.repo_name).into_iter().next();
            let mut o = Map::new();
            o.insert("repo_name".into(), json!(p.repo_name));
            if let Some(name) = &p.everyday_name {
                o.insert("everyday_name".into(), name.clone());
            }
            o.insert("current_brief".into(), json!(cur.map(|c| c.path)));
            Value::Object(o)
        })
        .collect()
}

fn prior_list(out_dir: &str, key: &str) -> Vec<Value> {
    history(out_dir, key)
        .into_iter()
        .rev()
        .map(|f| json!({ "number": f.number, "date": f.entry.parsed.as_ref().map(|p| p.date.clone()), "location": f.entry.location.as_str(), "path": f.entry.path }))
        .collect()
}

fn templates_json(env: &Env) -> Result<Value> {
    let mut m = Map::new();
    for t in current_templates(env)? {
        m.insert(t.kind.into(), json!({ "version": t.version, "path": t.path }));
    }
    Ok(Value::Object(m))
}

pub fn project_context(cfg: &Config, env: &Env, cwd: &str, date: &str) -> Result<Value> {
    let out_dir = require_output_dir(cfg)?;
    let info = repo_info(cwd)?;
    let registry = load_registry(&out_dir)?;
    let checkout = Checkout { repo_name: info.repo_name.clone(), main_path: Some(info.main_path.clone()), remote: info.remote.clone() };
    let found = match_checkout(&registry, &checkout, env);
    let repo_name = found.project.as_ref().map_or(info.repo_name.clone(), |p| p.repo_name.clone());
    let work = join(&[&cfg.cache_dir, "work", &repo_name, date]);
    let mut repo = info.to_json();
    repo["repo_name"] = json!(repo_name);
    let mut base = Map::new();
    base.insert("brf_version".into(), json!(brf_version()));
    base.insert("templates".into(), templates_json(env)?);
    base.insert("output_dir".into(), json!(out_dir));
    base.insert("registry".into(), json!(registry.path));
    base.insert("repo".into(), repo);
    base.insert("registered".into(), json!(found.project.is_some()));
    if let Some(conflict) = &found.conflict {
        let at = conflict.local_path.clone().or_else(|| conflict.remote.clone()).unwrap_or_else(|| "null".into());
        base.insert(
            "stop".into(),
            json!(format!(
                "The registry has a project named \"{}\", but it is a different checkout ({at}). This repository is not registered. Tell the owner; do not brief it.",
                conflict.repo_name
            )),
        );
        return Ok(Value::Object(base));
    }
    let Some(project) = &found.project else {
        base.insert(
            "stop".into(),
            json!(format!(
                "\"{repo_name}\" is not in the registry, so it does not get a brief. Tell the owner. Only the owner adds projects: asking for a brief, or typing /brf, is not a request to register one."
            )),
        );
        return Ok(Value::Object(base));
    };
    std::fs::create_dir_all(&work)?;
    let prior = prior_list(&out_dir, &repo_name);
    // An entry without an everyday name has no such field here either.
    if let Some(name) = &project.everyday_name {
        base.insert("everyday_name".into(), name.clone());
    }
    base.insert("date".into(), json!(date));
    base.insert("date_human".into(), json!(human_date(date)));
    base.insert("output_name".into(), json!(format_name(Kind::Project, Some(&repo_name), date, None, None, "html")));
    base.insert("brief_number".into(), json!(next_number(&out_dir, &repo_name)));
    let baseline = prior.first().cloned().unwrap_or(Value::Null);
    base.insert("prior_briefs".into(), json!(prior));
    base.insert("baseline".into(), baseline);
    base.insert("work_dir".into(), json!(work));
    base.insert(
        "portfolio".into(),
        json!(portfolio(&out_dir, &registry).into_iter().filter(|p| p["repo_name"] != json!(repo_name)).collect::<Vec<_>>()),
    );
    let mut tools = Map::new();
    tools.insert("gh".into(), json!(tool_version("gh", &["--version"])));
    tools.insert("gh_prefix".into(), json!(cfg.gh_user.as_ref().map_or(String::new(), |u| format!("GH_TOKEN=$(gh auth token -u {u}) "))));
    tools.insert("mister_clean".into(), mister_clean(env.get("PATH").unwrap_or("")));
    base.insert("tools".into(), Value::Object(tools));
    Ok(Value::Object(base))
}

pub fn meta_context(cfg: &Config, env: &Env, date: &str) -> Result<Value> {
    let out_dir = require_output_dir(cfg)?;
    let registry = load_registry(&out_dir)?;
    let work = join(&[&cfg.cache_dir, "work", "_meta", date]);
    std::fs::create_dir_all(&work)?;
    let projects = portfolio(&out_dir, &registry);
    let missing: Vec<Value> = projects.iter().filter(|p| p["current_brief"].is_null()).map(|p| p["repo_name"].clone()).collect();
    let mut m = Map::new();
    m.insert("brf_version".into(), json!(brf_version()));
    m.insert("templates".into(), templates_json(env)?);
    m.insert("output_dir".into(), json!(out_dir));
    m.insert("registry".into(), json!(registry.path));
    m.insert("date".into(), json!(date));
    m.insert("date_human".into(), json!(human_date(date)));
    m.insert("output_name".into(), json!(format_name(Kind::Meta, None, date, None, None, "html")));
    m.insert("brief_number".into(), json!(next_number(&out_dir, META_KEY)));
    m.insert("prior_meta".into(), json!(prior_list(&out_dir, META_KEY)));
    m.insert("projects".into(), json!(projects));
    m.insert("missing".into(), json!(missing));
    m.insert("work_dir".into(), json!(work));
    Ok(Value::Object(m))
}
