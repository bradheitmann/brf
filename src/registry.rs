// The registry: the closed list of projects the owner has asked to have briefed.
// It lives in the output folder as registry.json, so every machine that syncs the
// folder shares one list. brf never adds a project on its own; `brf register`
// records the owner's own words as the reason.
use crate::config::{Env, expand_home, write_json_atomic};
use crate::error::{Error, Result, io_at};
use crate::naming::REPO_NAME_RE;
use crate::util::{join, resolve, trim};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::path::Path;
use std::sync::LazyLock;

pub const REGISTRY_FILE: &str = "registry.json";
pub const REGISTRY_ABOUT: &str = "Projects that get briefs. Add one only when the owner names it and asks for it, with `brf register --reason \"<their words>\"`. Never search the computer for projects.";

pub fn empty_registry() -> Value {
    json!({ "brf_registry": 1, "about": REGISTRY_ABOUT, "projects": [] })
}

pub fn registry_path(out_dir: &str) -> String {
    join(&[out_dir, REGISTRY_FILE])
}

/// One registry entry. Fields other than these are kept as they are when the registry is rewritten.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Project {
    pub repo_name: String,
    pub everyday_name: Option<Value>,
    pub local_path: Option<String>,
    pub remote: Option<String>,
}

impl Project {
    pub fn named(name: &str) -> Self {
        Project { repo_name: name.into(), ..Default::default() }
    }
}

#[derive(Clone, Debug)]
pub struct Registry {
    pub path: String,
    pub exists: bool,
    /// The file's own fields, in file order.
    pub data: Map<String, Value>,
    pub projects: Vec<Project>,
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string)
}

pub fn load_registry(out_dir: &str) -> Result<Registry> {
    let path = registry_path(out_dir);
    if !Path::new(&path).exists() {
        let Value::Object(data) = empty_registry() else { unreachable!() };
        return Ok(Registry { path, exists: false, data, projects: vec![] });
    }
    let text = std::fs::read_to_string(&path).map_err(|e| io_at(e, "open", &path))?;
    let data = match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => m,
        Ok(_) => Map::new(),
        Err(err) => return Err(Error::brf("BAD_REGISTRY", format!("{path} is not valid JSON: {err}"))),
    };
    let Some(list) = data.get("projects").and_then(Value::as_array) else {
        return Err(Error::brf("BAD_REGISTRY", format!("{path} has no \"projects\" list.")));
    };
    let mut projects = Vec::new();
    for p in list {
        let name = p.get("repo_name").and_then(Value::as_str).unwrap_or("");
        let is_entry = !matches!(p, Value::Null | Value::Bool(false)) && p.as_str() != Some("") && p.as_f64() != Some(0.0);
        if !is_entry || !REPO_NAME_RE.is_match(name) {
            let shown = match p.get("repo_name") {
                _ if !is_entry => serde_json::to_string(p).unwrap_or_default(),
                Some(v) => serde_json::to_string(v).unwrap_or_default(),
                None => "undefined".into(),
            };
            return Err(Error::brf("BAD_REGISTRY", format!("{path} has an entry whose repo_name is not lower-case-hyphenated: {shown}")));
        }
        projects.push(Project {
            repo_name: name.to_string(),
            everyday_name: p.get("everyday_name").cloned(),
            local_path: str_field(p, "local_path"),
            remote: str_field(p, "remote"),
        });
    }
    Ok(Registry { path, exists: true, data, projects })
}

fn norm(p: &str, env: &Env) -> String {
    let abs = resolve(&[&expand_home(p, env)]);
    std::fs::canonicalize(&abs).map(|p| p.to_string_lossy().into_owned()).unwrap_or(abs)
}

fn same_path(a: Option<&str>, b: Option<&str>, env: &Env) -> bool {
    match (a, b) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => norm(a, env) == norm(b, env),
        _ => false,
    }
}

static WITH_SCHEME: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?i-u:[a-z][a-z0-9+.-]*)://(?:[^@/]*@)?[^/]+/([^\n\r\x{2028}\x{2029}]*)$").unwrap());
static SCP_LIKE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[^@/:]+@)?[^/:]+:([^\n\r\x{2028}\x{2029}]*)$").unwrap());

/// The whole repository path of a remote, after the host, lower case: "acme/tide" for
/// github.com/acme/tide, "orga/payments/_git/api" for an Azure repo. Scheme, user, host, port
/// and SSH host aliases are dropped, so every clone of one remote gives one key.
pub fn remote_key(url: &str) -> Option<String> {
    if url.is_empty() {
        return None;
    }
    let s = trim(url);
    let path = if let Some(c) = WITH_SCHEME.captures(s) {
        c[1].to_string()
    } else if let Some(c) = SCP_LIKE.captures(s) {
        c[1].to_string()
    } else {
        s.to_string()
    };
    let lower = path.to_lowercase();
    let lower = lower.trim_end_matches('/');
    let lower = lower.strip_suffix(".git").unwrap_or(lower);
    let lower = lower.trim_start_matches('/');
    // Hosts that spell one repository two ways: Azure DevOps (HTTPS "org/project/_git/repo",
    // SSH "v3/org/project/repo") and Bitbucket Server (HTTPS "scm/project/repo", SSH "project/repo").
    let mut parts: Vec<&str> = lower.split('/').collect();
    if parts.len() >= 3 && (parts[0] == "v3" || parts[0] == "scm") {
        parts.remove(0);
    }
    if let Some(g) = parts.iter().position(|p| *p == "_git") {
        if g > 0 && g == parts.len() - 2 {
            parts.remove(g);
        }
    }
    let key = parts.join("/");
    if key.is_empty() { None } else { Some(key) }
}

fn same_remote(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) if !a.is_empty() && !b.is_empty() => remote_key(a) == remote_key(b),
        _ => false,
    }
}

/// What a checkout is: its main folder, its remote and the name it would have.
#[derive(Clone, Debug, Default)]
pub struct Checkout {
    pub repo_name: String,
    pub main_path: Option<String>,
    pub remote: Option<String>,
}

/// Does this registry entry describe this checkout? An entry that records a path or a
/// remote must match one of them; a name alone is not enough to be the same project.
pub fn same_project(entry: &Project, info: &Checkout, env: &Env) -> bool {
    if entry.local_path.is_none() && entry.remote.is_none() {
        return true;
    }
    same_path(entry.local_path.as_deref(), info.main_path.as_deref(), env) || same_remote(entry.remote.as_deref(), info.remote.as_deref())
}

#[derive(Clone, Debug, Default)]
pub struct Match {
    pub project: Option<Project>,
    pub conflict: Option<Project>,
}

/// Find the entry for a checkout: by recorded path, then by recorded remote (another clone,
/// another computer), then by name when the entry's recorded identity also matches. `conflict`
/// names an entry that has this repository's name but belongs to a different checkout.
pub fn match_checkout(registry: &Registry, info: &Checkout, env: &Env) -> Match {
    if let Some(by_path) = registry.projects.iter().find(|p| same_path(p.local_path.as_deref(), info.main_path.as_deref(), env)) {
        // Same folder, different repository (another computer, or the folder was reused).
        if let (Some(a), Some(b)) = (&by_path.remote, &info.remote) {
            if !b.is_empty() && remote_key(a) != remote_key(b) {
                return Match { project: None, conflict: Some(by_path.clone()) };
            }
        }
        return Match { project: Some(by_path.clone()), conflict: None };
    }
    if let Some(key) = info.remote.as_deref().and_then(remote_key) {
        if let Some(by_remote) = registry.projects.iter().find(|p| p.remote.as_deref().and_then(remote_key).as_deref() == Some(key.as_str())) {
            return Match { project: Some(by_remote.clone()), conflict: None };
        }
    }
    let Some(by_name) = registry.projects.iter().find(|p| p.repo_name == info.repo_name) else {
        return Match::default();
    };
    if same_project(by_name, info, env) {
        Match { project: Some(by_name.clone()), conflict: None }
    } else {
        Match { project: None, conflict: Some(by_name.clone()) }
    }
}

/// Look a project up by the name in a brief's file name.
pub fn find_project<'a>(registry: &'a Registry, repo_name: &str) -> Option<&'a Project> {
    registry.projects.iter().find(|p| p.repo_name == repo_name)
}

#[derive(Clone, Debug, Default)]
pub struct NewEntry {
    pub repo_name: String,
    pub everyday_name: Option<String>,
    pub local_path: Option<String>,
    pub remote: Option<String>,
    pub added: Option<String>,
    pub reason: Option<String>,
}

pub fn add_project(out_dir: &str, entry: &NewEntry, env: &Env) -> Result<Value> {
    let reason = trim(entry.reason.as_deref().unwrap_or("")).to_string();
    if reason.chars().map(char::len_utf16).sum::<usize>() < 3 {
        return Err(Error::brf("NO_REASON", "A project is registered only when the owner asks. Pass their words: --reason \"<what they said>\"."));
    }
    if !REPO_NAME_RE.is_match(&entry.repo_name) {
        return Err(Error::brf("BAD_NAME", format!("\"{}\" is not a lower-case-hyphenated repository name.", entry.repo_name)));
    }
    let reg = load_registry(out_dir)?;
    let existing = reg.projects.iter().find(|p| {
        p.repo_name == entry.repo_name
            || same_path(p.local_path.as_deref(), entry.local_path.as_deref(), env)
            || same_remote(p.remote.as_deref(), entry.remote.as_deref())
    });
    if let Some(e) = existing {
        return Err(Error::brf("ALREADY_REGISTERED", format!("This project is already registered as \"{}\".", e.repo_name)));
    }
    let pick = |s: &Option<String>| s.as_ref().filter(|s| !s.is_empty()).cloned();
    let mut record = Map::new();
    record.insert("repo_name".into(), json!(entry.repo_name));
    record.insert("everyday_name".into(), json!(pick(&entry.everyday_name).unwrap_or_else(|| entry.repo_name.clone())));
    record.insert("local_path".into(), json!(pick(&entry.local_path)));
    record.insert("remote".into(), json!(pick(&entry.remote)));
    // An entry made without a date carries no "added" field at all.
    if let Some(added) = &entry.added {
        record.insert("added".into(), json!(added));
    }
    record.insert("reason".into(), json!(reason));
    let record = Value::Object(record);
    let mut data = reg.data.clone();
    data.remove("path");
    data.remove("exists");
    let mut list = data.get("projects").and_then(Value::as_array).cloned().unwrap_or_default();
    list.push(record.clone());
    data.insert("projects".into(), Value::Array(list));
    write_json_atomic(&reg.path, &Value::Object(data))?;
    Ok(record)
}
