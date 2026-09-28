// The render check. brf itself has no browser; `brf build --verify` hands the finished page to
// verify/verify.ts, a strict TypeScript script run by Node (22.18 or newer, or 22.6 and up with
// type stripping switched on) or Bun. The script finds Playwright with Chromium, never installs
// it, and exits 0 (clean), 1 (problems) or 3 (skipped).
use crate::config::Config;
use crate::error::Result;
use crate::templates::root;
use crate::util::join;
use std::path::Path;
use std::process::{Command, Stdio};

/// How the check ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Clean,
    Failed,
    Skipped,
}

impl Verdict {
    pub fn exit_code(self) -> i32 {
        match self {
            Verdict::Clean => 0,
            Verdict::Failed => 1,
            Verdict::Skipped => 3,
        }
    }
}

fn version_of(cmd: &str) -> Option<String> {
    let out = Command::new(cmd).arg("--version").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The command that runs a .ts file here: node with built-in type stripping, else bun.
pub fn ts_runtime() -> Option<Vec<String>> {
    if let Some(v) = version_of("node") {
        let nums: Vec<u32> = v.trim_start_matches('v').split('.').filter_map(|p| p.parse().ok()).collect();
        let (major, minor) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0));
        // Type stripping is on by default from 22.18 and 23.6; from 22.6 it needs a flag.
        let native = major >= 24 || (major == 23 && minor >= 6) || (major == 22 && minor >= 18);
        let flagged = (major == 22 && minor >= 6) || major == 23;
        if native {
            return Some(vec!["node".into()]);
        }
        if flagged {
            return Some(vec!["node".into(), "--experimental-strip-types".into(), "--disable-warning=ExperimentalWarning".into()]);
        }
    }
    version_of("bun").map(|_| vec!["bun".into()])
}

pub fn script_path() -> String {
    join(&[root(), "verify", "verify.ts"])
}

/// Render `file` at five widths and write screenshots and a report to `proof_dir`.
/// Output from the script goes straight to this process's output.
pub fn verify(file: &str, proof_dir: &str, cfg: &Config, log: &mut dyn FnMut(&str)) -> Result<Verdict> {
    let script = script_path();
    let Some(runtime) = ts_runtime().filter(|_| Path::new(&script).is_file()) else {
        log("proof     SKIPPED. The render check needs Node 22.18 or newer, or Bun, to run its script:");
        log(&format!("          {script}"));
        return Ok(Verdict::Skipped);
    };
    let mut cmd = Command::new(&runtime[0]);
    cmd.args(&runtime[1..]).arg(&script).args(["--file", file, "--proof-dir", proof_dir, "--root", root()]);
    if let Some(pw) = &cfg.playwright {
        cmd.args(["--playwright", pw]);
    }
    let status = cmd.stdin(Stdio::null()).status()?;
    Ok(match status.code() {
        Some(0) => Verdict::Clean,
        Some(3) => Verdict::Skipped,
        _ => Verdict::Failed,
    })
}
