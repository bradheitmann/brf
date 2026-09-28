// Read-only facts about the git repository the agent is standing in.
// Every git call passes --no-optional-locks, so even `git status` never rewrites the index.
use crate::error::{Error, Result};
use crate::naming::to_repo_name;
use crate::util::{basename, dirname, is_absolute, resolve};
use regex::Regex;
use serde_json::{Value, json};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

pub fn git(cwd: &str, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("--no-optional-locks")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    Some(s.strip_suffix('\n').map(str::to_string).unwrap_or(s))
}

static CREDENTIALS_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^((?i-u:[a-z][a-z0-9+.-]*)://)[^@/]+@").unwrap());

/// Remote URLs can carry a user name or token. Never pass one on.
pub fn redact_remote(url: &str) -> String {
    CREDENTIALS_RE.replace(url, "${1}").into_owned()
}

pub fn name_from_remote(url: &str) -> Option<String> {
    let trimmed = url.trim_end_matches(['/', '\\']);
    let last = trimmed.rsplit(['/', ':']).next().unwrap_or("");
    let name = to_repo_name(last);
    if name.is_empty() { None } else { Some(name) }
}

#[derive(Clone, Debug)]
pub struct RepoInfo {
    pub toplevel: String,
    pub main_path: String,
    pub is_linked_worktree: bool,
    pub remote: Option<String>,
    pub repo_name: String,
    pub repo_name_source: String,
    pub head: Option<String>,
    pub branch: Option<String>,
}

impl RepoInfo {
    pub fn to_json(&self) -> Value {
        json!({
            "toplevel": self.toplevel,
            "main_path": self.main_path,
            "is_linked_worktree": self.is_linked_worktree,
            "remote": self.remote,
            "repo_name": self.repo_name,
            "repo_name_source": self.repo_name_source,
            "head": self.head,
            "branch": self.branch,
        })
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|s| !s.is_empty())
}

pub fn repo_info(cwd: &str) -> Result<RepoInfo> {
    let Some(toplevel) = non_empty(git(cwd, &["rev-parse", "--show-toplevel"])) else {
        return Err(Error::brf("NOT_A_REPO", format!("{cwd} is not inside a git repository. Run brf from the project you want briefed.")));
    };
    let mut common = non_empty(git(&toplevel, &["rev-parse", "--git-common-dir"]));
    if let Some(c) = &common {
        if !is_absolute(c) {
            common = Some(resolve(&[&toplevel, c]));
        }
    }
    // In a linked worktree the common dir is the main checkout's .git. Brief the project, not the worktree.
    let main = match &common {
        Some(c) if basename(c) == ".git" => dirname(c),
        _ => toplevel.clone(),
    };
    let remotes: Vec<String> = git(&toplevel, &["remote"]).unwrap_or_default().split('\n').filter(|s| !s.is_empty()).map(str::to_string).collect();
    let remote_name = if remotes.iter().any(|r| r == "origin") { Some("origin".to_string()) } else { remotes.first().cloned() };
    let remote = remote_name.as_ref().and_then(|n| non_empty(git(&toplevel, &["remote", "get-url", n]))).map(|u| redact_remote(&u));
    let from_remote = remote.as_deref().and_then(name_from_remote);
    let from_folder = to_repo_name(&basename(&main));
    let main_path = std::fs::canonicalize(&main).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| main.clone());
    Ok(RepoInfo {
        is_linked_worktree: resolve(&[&toplevel]) != resolve(&[&main]),
        repo_name_source: match (&from_remote, &remote_name) {
            (Some(_), Some(n)) => format!("remote {n}"),
            _ => "folder name".into(),
        },
        repo_name: from_remote.unwrap_or(from_folder),
        head: non_empty(git(&toplevel, &["rev-parse", "HEAD"])),
        branch: non_empty(git(&toplevel, &["rev-parse", "--abbrev-ref", "HEAD"])),
        toplevel,
        main_path,
        remote,
    })
}
