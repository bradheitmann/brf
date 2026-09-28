// Proof that briefing a repository changed nothing in it.
//
// A snapshot hashes everything an agent could disturb: HEAD and the current branch, staged
// and unstaged changes (their content, not only their paths), the content of untracked files,
// ignored paths, every ref (branches, tags, remote-tracking refs, the stash), the stash list,
// the worktree list, and HEAD's reflog. All of it is read with --no-optional-locks and nothing is written.
use crate::config::write_json_atomic;
use crate::error::{Error, Result, io_at};
use crate::repo::repo_info;
use crate::util::{hex, iso_now, join, sort_utf16};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::os::unix::fs::MetadataExt;
use std::process::{Command, Stdio};

fn git_bytes(cwd: &str, args: &[&str], allow_fail: bool) -> Result<Vec<u8>> {
    // log.showSignature would make log-style reads verify every signature: slow, and its text
    // can change when nothing in the repository did.
    let out = Command::new("git")
        .args(["--no-optional-locks", "-c", "log.showSignature=false"])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output();
    match out {
        Ok(o) if o.status.success() => Ok(o.stdout),
        _ if allow_fail => Ok(Vec::new()),
        _ => Err(Error::brf("GIT_FAILED", format!("git {} failed; no snapshot taken.", args.join(" ")))),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn untracked_digest(cwd: &str) -> Result<(String, usize)> {
    let mut h = Sha256::new();
    let raw = git_bytes(cwd, &["ls-files", "--others", "--exclude-standard", "-z"], false)?;
    let mut list: Vec<String> = String::from_utf8_lossy(&raw).split('\0').filter(|s| !s.is_empty()).map(str::to_string).collect();
    sort_utf16(&mut list);
    for rel in &list {
        let p = join(&[cwd, rel]);
        h.update(format!("{rel}\0").as_bytes());
        match std::fs::symlink_metadata(&p) {
            Ok(st) if st.file_type().is_symlink() => match std::fs::read_link(&p) {
                Ok(target) => h.update(format!("link:{}", target.to_string_lossy()).as_bytes()),
                Err(_) => h.update(b"unreadable"),
            },
            Ok(st) if st.is_file() => match std::fs::read(&p) {
                Ok(bytes) => h.update(&bytes),
                Err(_) => h.update(b"unreadable"),
            },
            Ok(st) => h.update(format!("other:{}", st.mode()).as_bytes()),
            Err(_) => h.update(b"unreadable"),
        }
        h.update(b"\0");
    }
    Ok((hex(&h.finalize()), list.len()))
}

const PARTS: [&str; 9] = ["head", "branch", "status", "staged", "unstaged", "refs", "stashes", "worktrees", "reflog"];

pub fn take_snapshot(cwd: &str) -> Result<Value> {
    let info = repo_info(cwd)?;
    let top = info.toplevel.as_str();
    let has_head = info.head.is_some();
    let branch = git_bytes(top, &["symbolic-ref", "-q", "HEAD"], true)?;
    let parts: [Vec<u8>; 9] = [
        info.head.clone().unwrap_or_else(|| "unborn".into()).into_bytes(),
        branch.clone(),
        git_bytes(top, &["status", "--porcelain=v1", "-z", "--ignored"], false)?,
        git_bytes(top, if has_head { &["diff", "--cached", "--binary", "HEAD"] } else { &["diff", "--cached", "--binary"] }, false)?,
        git_bytes(top, &["diff", "--binary"], false)?,
        git_bytes(top, &["for-each-ref", "--format=%(refname) %(objectname)"], false)?,
        git_bytes(top, &["stash", "list", "--format=%H"], true)?,
        git_bytes(top, &["worktree", "list", "--porcelain"], false)?,
        // Round trips (switch away and back, stash then pop) leave the tree as it was but write
        // HEAD's reflog, so the reflog is part of the proof.
        git_bytes(top, &["reflog", "show", "--format=%H%x00%gs", "HEAD"], true)?,
    ];
    let (untracked, count) = untracked_digest(top)?;
    let mut combined = Sha256::new();
    let mut digests = Map::new();
    for (k, v) in PARTS.iter().zip(parts.iter()) {
        let d = sha256_hex(v);
        combined.update(format!("{k}:{d}\n").as_bytes());
        digests.insert((*k).into(), json!(d));
    }
    digests.insert("untracked_content".into(), json!(untracked));
    combined.update(format!("untracked_content:{untracked}\n").as_bytes());
    let repo = std::fs::canonicalize(top).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| top.to_string());
    let branch = String::from_utf8_lossy(&branch);
    let branch = crate::util::trim(&branch);
    Ok(json!({
        "brf_snapshot": 1,
        "repo": repo,
        "head": info.head,
        "branch": if branch.is_empty() { Value::Null } else { json!(branch) },
        "untracked_files": count,
        "digests": digests,
        "digest": hex(&combined.finalize()),
        "taken_at": iso_now(),
    }))
}

pub fn save_snapshot(path: &str, snap: &Value) -> Result<()> {
    write_json_atomic(path, snap)
}

#[derive(Clone, Debug)]
pub struct Comparison {
    pub same: bool,
    pub changed: Vec<String>,
    pub before: Value,
    pub after: Value,
}

pub fn compare_snapshot(before_path: &str, cwd: &str) -> Result<Comparison> {
    let text = std::fs::read_to_string(before_path)
        .map_err(|e| Error::brf("NO_SNAPSHOT", format!("Cannot read the earlier snapshot {before_path}: {}", io_at(e, "open", before_path))))?;
    let before: Value =
        serde_json::from_str(&text).map_err(|e| Error::brf("NO_SNAPSHOT", format!("Cannot read the earlier snapshot {before_path}: {e}")))?;
    let truthy = |v: Option<&Value>| match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        _ => true,
    };
    if before.get("brf_snapshot").and_then(Value::as_f64) != Some(1.0) || !truthy(before.get("digest")) {
        return Err(Error::brf("OLD_SNAPSHOT", format!("{before_path} was not made by this version of brf snapshot. Take a new one.")));
    }
    let after = take_snapshot(cwd)?;
    if before.get("repo") != after.get("repo") {
        let shown = match before.get("repo") {
            Some(Value::String(s)) => s.clone(),
            Some(v) => v.to_string(),
            None => "undefined".into(),
        };
        return Err(Error::brf("OTHER_REPO", format!("The snapshot was taken in {shown}, not {}.", after["repo"].as_str().unwrap_or(""))));
    }
    let empty = Map::new();
    let after_digests = after["digests"].as_object().unwrap_or(&empty);
    let before_digests = before.get("digests").and_then(Value::as_object);
    let changed: Vec<String> =
        after_digests.iter().filter(|(k, v)| before_digests.and_then(|b| b.get(k.as_str())) != Some(*v)).map(|(k, _)| k.clone()).collect();
    let same = changed.is_empty() && before.get("digest") == after.get("digest");
    Ok(Comparison { same, changed, before, after })
}
