// Put a finished brief in the output folder and move that project's previous brief
// into archive/. Only files of the same project are touched. Nothing is overwritten
// and nothing is deleted.
use crate::check::{markup_problems, stamp_mismatch};
use crate::error::{Error, Result, io_at};
use crate::folder::{archive_dir, current_files, ensure_layout, next_number};
use crate::html::read_stamp;
use crate::naming::{Kind, Parsed, format_parsed, parse_name};
use crate::registry::{find_project, load_registry};
use crate::util::{basename, hex, join};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

/// A lock per output folder, kept on this machine, so two agents delivering at once take turns.
pub fn with_lock<T>(cache_dir: &str, out_dir: &str, wait: Duration, stale: Duration, f: impl FnOnce() -> Result<T>) -> Result<T> {
    let locks = join(&[cache_dir, "locks"]);
    let lock = join(&[&locks, &format!("{}.lock", &hex(&Sha256::digest(out_dir.as_bytes()))[..16])]);
    std::fs::create_dir_all(&locks)?;
    let deadline = Instant::now() + wait;
    loop {
        match std::fs::create_dir(&lock) {
            Ok(()) => break,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let age = std::fs::metadata(&lock).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
                if age.is_some_and(|a| a > stale) {
                    let _ = std::fs::remove_dir(&lock);
                    continue;
                }
                if Instant::now() > deadline {
                    return Err(Error::brf("LOCKED", format!("Another delivery holds {lock}. Try again in a minute.")));
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(e.into()),
        }
    }
    let result = f();
    let _ = std::fs::remove_dir(&lock);
    result
}

pub const LOCK_WAIT: Duration = Duration::from_secs(15);
pub const LOCK_STALE: Duration = Duration::from_secs(120);

/// The archive name for a file leaving the root. A same-day name that is taken, or that
/// the incoming brief will use, becomes -v1, -v2 … in the order the briefs were made.
pub fn archive_name_for(out_dir: &str, parsed: &Parsed, incoming_date: &str) -> Result<String> {
    let arch = archive_dir(out_dir);
    let plain = format_parsed(parsed, None);
    if parsed.date != incoming_date && !Path::new(&join(&[&arch, &plain])).exists() {
        return Ok(plain);
    }
    for v in 1..1000 {
        let name = format_parsed(parsed, Some(v));
        if !Path::new(&join(&[&arch, &name])).exists() {
            return Ok(name);
        }
    }
    Err(Error::brf("ARCHIVE_FULL", format!("Too many same-day copies of {plain}.")))
}

pub fn deliver(file: &str, output_dir: &str, cache_dir: &str) -> Result<Value> {
    let name = basename(file);
    let parsed = match parse_name(&name) {
        Some(p) if p.ext == "html" && !p.has_v() && p.variant.is_none() => p,
        _ => return Err(Error::brf("BAD_NAME", format!("\"{name}\" is not a deliverable brief name."))),
    };
    if !Path::new(file).exists() {
        return Err(Error::brf("NOT_FOUND", format!("Not found: {file}")));
    }
    let html = String::from_utf8_lossy(&std::fs::read(file).map_err(|e| io_at(e, "open", file))?).into_owned();
    let stamp = read_stamp(&html);
    let filled = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.is_empty());
    let brief = stamp.brief.filter(|b| *b != 0);
    let Some(brief) = brief.filter(|_| filled(&stamp.brf_version) && filled(&stamp.template)) else {
        return Err(Error::brf("NOT_BUILT", format!("{name} has no complete brf stamp. Deliver only what brf build produced.")));
    };
    let markup = markup_problems(&html);
    if !markup.is_empty() {
        return Err(Error::brf("UNSAFE_MARKUP", format!("{name} {}. Remove it, rebuild, and run brf check.", markup.join("; "))));
    }
    let mismatch = stamp_mismatch(&stamp, Some(&parsed));
    if !mismatch.is_empty() {
        return Err(Error::brf("STAMP_MISMATCH", format!("{name} {}. Deliver the file under the name brf build gave it.", mismatch.join("; "))));
    }
    if parsed.kind == Kind::Project {
        let repo = parsed.repo.as_deref().unwrap_or("");
        if find_project(&load_registry(output_dir)?, repo).is_none() {
            return Err(Error::brf(
                "NOT_REGISTERED",
                format!("\"{repo}\" is not in the registry. Briefs are made only for projects the owner has asked for."),
            ));
        }
    }
    ensure_layout(output_dir)?;
    with_lock(cache_dir, output_dir, LOCK_WAIT, LOCK_STALE, || {
        let expected = next_number(output_dir, &parsed.key);
        if brief != expected {
            return Err(Error::brf(
                "BRIEF_NUMBER",
                format!("{name} is stamped brief {brief}, but the next brief for this project is {expected}. Another brief landed first; rebuild."),
            ));
        }
        let mut moved = Vec::new();
        for f in current_files(output_dir, &parsed.key) {
            let fp = f.parsed.as_ref().expect("a current brief has a parsed name");
            if fp.date > parsed.date {
                return Err(Error::brf("NEWER_EXISTS", format!("{} describes a later date than {name}. Refusing to archive a newer brief.", f.name)));
            }
            let target = join(&[&archive_dir(output_dir), &archive_name_for(output_dir, fp, &parsed.date)?]);
            if Path::new(&target).exists() {
                return Err(Error::brf("EXISTS", format!("{target} already exists.")));
            }
            std::fs::rename(&f.path, &target)?;
            moved.push(json!({ "from": f.name, "to": format!("archive/{}", basename(&target)) }));
        }
        let dest = join(&[output_dir, &name]);
        // Never overwrite: the copy fails if the name is already taken.
        let mut src = std::fs::File::open(file)?;
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(|e| io_at(e, "copyfile", &dest))?;
        std::io::copy(&mut src, &mut out)?;
        Ok(json!({ "delivered": dest, "brief": brief, "archived": moved }))
    })
}
