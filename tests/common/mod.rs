// Shared test helpers: temporary folders, throwaway git repositories, and the command line run
// with an isolated config and cache. Nothing here reads or writes the owner's real brief folder.
#![allow(dead_code)]

use brf::build::{BuildOptions, Built, build};
use brf::config::Env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

pub const ROOT: &str = env!("CARGO_MANIFEST_DIR");
pub const BIN: &str = env!("CARGO_BIN_EXE_brf");

pub fn root_file(rel: &str) -> String {
    format!("{ROOT}/{rel}")
}

pub fn example() -> String {
    root_file("examples/tidewater.filled.html")
}

/// A folder removed when the value is dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn path(&self) -> &str {
        self.0.to_str().expect("utf-8 temp path")
    }

    pub fn join(&self, rel: &str) -> String {
        format!("{}/{rel}", self.path())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn temp_dir() -> TempDir {
    let base = std::env::temp_dir();
    let base = base.to_str().expect("utf-8 temp dir").trim_end_matches('/');
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos();
    let dir = PathBuf::from(format!("{base}/brf-test-{}-{}-{nanos}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)));
    std::fs::create_dir_all(&dir).unwrap();
    TempDir(dir)
}

pub fn git(cwd: &str, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.email=brf-test", "-c", "user.name=test", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed in {cwd}");
}

pub fn git_out(cwd: &str, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(cwd).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

pub fn git_repo(dir: &str, remote: Option<&str>) -> String {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q"]);
    if let Some(r) = remote {
        git(dir, &["remote", "add", "origin", r]);
    }
    std::fs::write(Path::new(dir).join("README.md"), "test\n").unwrap();
    git(dir, &["add", "README.md"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    dir.to_string()
}

pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}\n{}", self.stdout, self.stderr))
    }
}

/// The variables every command-line run gets, so no run sees the owner's own settings.
pub fn isolated_env(home: &str) -> Vec<(String, String)> {
    vec![
        ("XDG_CONFIG_HOME".into(), format!("{home}/cfg")),
        ("XDG_CACHE_HOME".into(), format!("{home}/cache")),
        ("BRF_OUTPUT_DIR".into(), String::new()),
        ("BRF_CONFIG".into(), String::new()),
        ("BRF_TEMPLATES_DIR".into(), String::new()),
        ("BRF_PLAYWRIGHT".into(), String::new()),
        ("BRF_GH_USER".into(), String::new()),
    ]
}

/// Run a program with an isolated config and cache.
pub fn run_with(program: &str, pre: &[&str], args: &[&str], cwd: &str, home: &str, extra: &[(&str, &str)]) -> Run {
    let mut cmd = Command::new(program);
    cmd.args(pre).args(args).current_dir(cwd).stdin(Stdio::null());
    for (k, v) in isolated_env(home) {
        cmd.env(k, v);
    }
    for (k, v) in extra {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// Run the brf binary with an isolated config and cache.
pub fn cli(args: &[&str], cwd: &str, home: &str) -> Run {
    run_with(BIN, &[], args, cwd, home, &[])
}

pub fn cli_env(args: &[&str], cwd: &str, home: &str, extra: &[(&str, &str)]) -> Run {
    run_with(BIN, &[], args, cwd, home, extra)
}

/// An environment for library calls: an empty home, so no owner config or templates are seen.
pub fn test_env(home: &str) -> Env {
    Env::from_pairs(&[
        ("HOME", home),
        ("XDG_CONFIG_HOME", &format!("{home}/cfg")),
        ("XDG_CACHE_HOME", &format!("{home}/cache")),
        ("PATH", &std::env::var("PATH").unwrap_or_default()),
    ])
}

pub fn opts<'a>(input: &'a str, out: &'a str, env: &'a Env) -> BuildOptions<'a> {
    BuildOptions { input, out, brief: None, example: false, output_dir: None, env }
}

/// Build quietly.
pub fn build_quiet(o: &BuildOptions<'_>) -> brf::error::Result<Built> {
    build(o, &mut |_| {})
}

pub fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub fn write(path: &str, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{path}: {e}"));
}

/// The error message of a failed result, for matching.
pub fn err_text<T: std::fmt::Debug>(r: brf::error::Result<T>) -> String {
    match r {
        Ok(v) => panic!("expected an error, got {v:?}"),
        Err(e) => format!("{e} ({})", e.code().unwrap_or("")),
    }
}

pub fn write_registry(out: &str, projects: serde_json::Value) {
    let mut reg = brf::registry::empty_registry();
    reg["projects"] = projects;
    brf::config::write_json_atomic(&brf::registry::registry_path(out), &reg).unwrap();
}

/// The example brief, built as a sample page, as text.
pub fn built_example(env: &Env, brief: Option<f64>) -> String {
    let dir = temp_dir();
    let out = dir.join("brf_tidewater_20261102.html");
    let ex = example();
    let r = build_quiet(&BuildOptions { input: &ex, out: &out, brief, example: true, output_dir: None, env }).unwrap();
    read(&r.path)
}

pub fn re(p: &str) -> regex::Regex {
    regex::Regex::new(p).unwrap()
}
