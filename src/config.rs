// Where brf keeps its settings, and how it finds the owner's output folder.
//
// Resolution order for every setting: environment variable, then the config file.
//   BRF_OUTPUT_DIR     the folder that receives briefs (required before anything is delivered)
//   BRF_PLAYWRIGHT     a path from which the playwright package can be resolved (optional)
//   BRF_GH_USER        the gh account that can read the owner's repositories (optional)
//   BRF_TEMPLATES_DIR  a folder with the owner's own templates (optional)
//   BRF_CONFIG         an alternative config file path (optional)
// The config file lives at $XDG_CONFIG_HOME/brf/config.json, or ~/.config/brf/config.json.
use crate::error::{Error, Result, io_at};
use crate::util::{dirname, join, resolve};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::Path;

/// The environment brf reads. Captured once from the process; tests build their own.
#[derive(Clone, Debug, Default)]
pub struct Env {
    vars: HashMap<String, String>,
}

impl Env {
    pub fn process() -> Self {
        let mut vars: HashMap<String, String> =
            std::env::vars_os().filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?))).collect();
        // home_dir is deprecated only for its Windows behaviour; brf runs on macOS and Linux.
        #[allow(deprecated)]
        if vars.get("HOME").is_none_or(|h| h.is_empty()) {
            if let Some(h) = std::env::home_dir() {
                vars.insert("HOME".into(), h.to_string_lossy().into_owned());
            }
        }
        Env { vars }
    }

    /// An environment with only these variables.
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Env { vars: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.vars.insert(key.into(), value.into());
    }

    /// A variable, when it is set and not empty.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str).filter(|v| !v.is_empty())
    }

    pub fn home(&self) -> String {
        self.get("HOME").unwrap_or("/").to_string()
    }
}

pub fn expand_home(p: &str, env: &Env) -> String {
    if p == "~" {
        return env.home();
    }
    match p.strip_prefix("~/") {
        Some(rest) => join(&[&env.home(), rest]),
        None => p.to_string(),
    }
}

pub fn config_path(env: &Env) -> String {
    if let Some(p) = env.get("BRF_CONFIG") {
        return resolve(&[&expand_home(p, env)]);
    }
    let base = env.get("XDG_CONFIG_HOME").map(str::to_string).unwrap_or_else(|| join(&[&env.home(), ".config"]));
    join(&[&base, "brf", "config.json"])
}

pub fn cache_dir(env: &Env) -> String {
    let base = env.get("XDG_CACHE_HOME").map(str::to_string).unwrap_or_else(|| join(&[&env.home(), ".cache"]));
    join(&[&base, "brf"])
}

/// The config file as an object; empty when there is none.
pub fn read_config_file(path: &str) -> Result<Map<String, Value>> {
    if !Path::new(path).exists() {
        return Ok(Map::new());
    }
    let text = std::fs::read_to_string(path).map_err(|e| io_at(e, "open", path))?;
    match serde_json::from_str::<Value>(&text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Ok(Map::new()),
        Err(err) => Err(Error::brf("BAD_CONFIG", format!("The config file {path} is not valid JSON: {err}"))),
    }
}

/// A string setting from the config file, when it is a non-empty string.
pub fn file_str<'a>(file: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    file.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

#[derive(Clone, Debug)]
pub struct Config {
    pub config_path: String,
    pub config_exists: bool,
    pub output_dir: Option<String>,
    pub output_dir_source: Option<&'static str>,
    pub playwright: Option<String>,
    pub gh_user: Option<String>,
    pub extra_banned_words: Vec<Value>,
    pub cache_dir: String,
}

impl Config {
    pub fn to_json(&self) -> Value {
        json!({
            "config_path": self.config_path,
            "config_exists": self.config_exists,
            "output_dir": self.output_dir,
            "output_dir_source": self.output_dir_source,
            "playwright": self.playwright,
            "gh_user": self.gh_user,
            "extra_banned_words": self.extra_banned_words,
            "cache_dir": self.cache_dir,
        })
    }

    /// The owner's extra banned words that are words (strings or numbers).
    pub fn banned_extra(&self) -> Vec<String> {
        self.extra_banned_words
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.clone()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .collect()
    }
}

pub fn load_config(env: &Env) -> Result<Config> {
    let path = config_path(env);
    let file = read_config_file(&path)?;
    let out = env.get("BRF_OUTPUT_DIR").or_else(|| file_str(&file, "output_dir"));
    Ok(Config {
        config_exists: Path::new(&path).exists(),
        output_dir: out.map(|o| resolve(&[&expand_home(o, env)])),
        output_dir_source: if env.get("BRF_OUTPUT_DIR").is_some() {
            Some("BRF_OUTPUT_DIR")
        } else if file_str(&file, "output_dir").is_some() {
            Some("config file")
        } else {
            None
        },
        playwright: env.get("BRF_PLAYWRIGHT").or_else(|| file_str(&file, "playwright")).map(str::to_string),
        gh_user: env.get("BRF_GH_USER").or_else(|| file_str(&file, "gh_user")).map(str::to_string),
        extra_banned_words: file.get("extra_banned_words").and_then(Value::as_array).cloned().unwrap_or_default(),
        cache_dir: cache_dir(env),
        config_path: path,
    })
}

pub fn require_output_dir(cfg: &Config) -> Result<String> {
    let Some(dir) = &cfg.output_dir else {
        return Err(Error::brf(
            "NO_OUTPUT_DIR",
            "No output folder is set. The owner chooses one, then runs:\n  brf init --output <folder>\n(or sets BRF_OUTPUT_DIR for this shell).",
        ));
    };
    if !Path::new(dir).exists() {
        return Err(Error::brf(
            "OUTPUT_DIR_MISSING",
            format!(
                "The output folder {dir} is not there. It may be on a drive or sync service that is not available yet. Tell the owner; only the owner runs brf init."
            ),
        ));
    }
    Ok(dir.clone())
}

/// Write JSON through a temporary file so a crash never leaves half a file behind.
pub fn write_json_atomic(path: &str, value: &Value) -> Result<()> {
    std::fs::create_dir_all(dirname(path))?;
    let tmp = format!("{path}.{}.tmp", std::process::id());
    std::fs::write(&tmp, format!("{}\n", crate::util::pretty(value)))?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}
