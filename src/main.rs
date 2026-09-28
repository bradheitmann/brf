// brf: re-entry briefs for the projects you own. See README.md and PROTOCOL.md.
use brf::build::{BuildOptions, build};
use brf::check::{check_file, check_folder_today};
use brf::config::{Config, Env, expand_home, load_config, read_config_file, require_output_dir, write_json_atomic};
use brf::context::{meta_context, project_context};
use brf::deliver::deliver;
use brf::error::{Error, Result};
use brf::folder::ensure_layout;
use brf::naming::{Kind, parse_name, to_repo_name, today_stamp};
use brf::registry::{Checkout, NewEntry, add_project, empty_registry, load_registry, match_checkout, registry_path};
use brf::repo::repo_info;
use brf::snapshot::{compare_snapshot, save_snapshot, take_snapshot};
use brf::templates::{brf_version, current_templates};
use brf::util::{basename, cwd, dirname, parse_number, pretty, resolve};
use brf::verify::verify;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

fn help() -> String {
    format!(
        r#"brf {}: re-entry briefs, one phone-readable page per project.

  brf init --output <folder> [--playwright <path>] [--gh-user <name>] [--templates-dir <folder>] [--force]
      Set up the output folder (registry.json, archive/) and point this machine at it.
  brf config                       Show the settings brf resolved.
  brf context [--meta]             JSON: this repo's name, registration, file name, brief number,
                                   earlier briefs, work folder. --meta for the meta brief.
  brf register --reason "<the owner's words>" [--everyday-name <name>] [--repo-name <name>]
                                   Add this repo to the registry. Only when the owner explicitly
                                   asked to add it; asking for a brief is not that request.
  brf snapshot --out <file>        Record HEAD and a hash of git status (read-only proof).
  brf snapshot --compare <file>    Exit 1 if the repo changed since that snapshot.
  brf build <filled.html> --out <work_dir>/<brf_repo_yyyymmdd.html> [--verify] [--proof-dir <dir>]
            [--brief <n>] [--example]
                                   Embed fonts, refuse unfilled slots, stamp versions, render-check.
                                   Exit 3 means the render check was skipped (Playwright or
                                   Chromium is missing or would not start).
  brf deliver <built.html>         File it in the output folder; the project's previous brief
                                   moves to archive/.
  brf check [--file <brief.html>]  Check one brief, or the whole output folder.
  brf version                      Print the brf and template versions.
"#,
        brf_version()
    )
}

#[derive(Clone, Debug)]
enum Flag {
    On,
    Text(String),
}

struct Args {
    flags: HashMap<String, Flag>,
    positional: Vec<String>,
}

impl Args {
    /// A flag given with a non-empty value, or given bare.
    fn on(&self, key: &str) -> bool {
        match self.flags.get(key) {
            Some(Flag::On) => true,
            Some(Flag::Text(s)) => !s.is_empty(),
            None => false,
        }
    }

    /// A flag's text, when it has non-empty text.
    fn text(&self, key: &str) -> Option<&str> {
        match self.flags.get(key) {
            Some(Flag::Text(s)) if !s.is_empty() => Some(s),
            _ => None,
        }
    }

    /// A flag's text, even when empty. A bare flag reads as "true".
    fn raw(&self, key: &str) -> Option<&str> {
        match self.flags.get(key) {
            Some(Flag::Text(s)) => Some(s),
            Some(Flag::On) => Some("true"),
            None => None,
        }
    }
}

fn parse_args(argv: &[String]) -> Result<Args> {
    let bools = ["verify", "example", "meta", "force", "json", "help"];
    let mut flags = HashMap::new();
    let mut positional = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if let Some(body) = a.strip_prefix("--") {
            if let Some(eq) = body.find('=').filter(|&e| e > 0) {
                flags.insert(body[..eq].to_string(), Flag::Text(body[eq + 1..].to_string()));
                i += 1;
                continue;
            }
            if bools.contains(&body) {
                flags.insert(body.to_string(), Flag::On);
            } else {
                match argv.get(i + 1) {
                    Some(v) if !v.starts_with("--") => {
                        flags.insert(body.to_string(), Flag::Text(v.clone()));
                        i += 1;
                    }
                    _ => return Err(Error::brf("MISSING_VALUE", format!("--{body} needs a value."))),
                }
            }
        } else {
            positional.push(a.clone());
        }
        i += 1;
    }
    Ok(Args { flags, positional })
}

/// Resolve symlinks and letter case on the nearest existing ancestor, so /var and /private/var,
/// or two spellings of one folder on a case-insensitive disk, compare equal.
fn real_loose(p: &str) -> String {
    let mut dir = p.to_string();
    let mut tail: Vec<String> = Vec::new();
    while !Path::new(&dir).exists() && dirname(&dir) != dir {
        tail.insert(0, basename(&dir));
        dir = dirname(&dir);
    }
    let real = std::fs::canonicalize(&dir).map(|p| p.to_string_lossy().into_owned()).unwrap_or(dir);
    std::iter::once(real).chain(tail).collect::<Vec<_>>().join("/")
}

fn is_inside(path: &str, root: &str) -> bool {
    let r = real_loose(root);
    let p = real_loose(path);
    p == r || p.starts_with(&format!("{r}/"))
}

fn checkout_of(info: &brf::repo::RepoInfo) -> Checkout {
    Checkout { repo_name: info.repo_name.clone(), main_path: Some(info.main_path.clone()), remote: info.remote.clone() }
}

/// Run inside a repository, build and deliver make sure a project brief belongs to this checkout,
/// not to a different one that happens to share its name.
fn checkout_guard(file: &str, output_dir: &str, env: &Env) -> Result<()> {
    let Some(parsed) = parse_name(&basename(file)).filter(|p| p.kind == Kind::Project) else { return Ok(()) };
    let Ok(info) = repo_info(&cwd()) else { return Ok(()) };
    let found = match_checkout(&load_registry(output_dir)?, &checkout_of(&info), env);
    if let Some(c) = found.conflict {
        let at = c.local_path.or(c.remote).unwrap_or_default();
        return Err(Error::brf("OTHER_CHECKOUT", format!("This checkout is not the registered \"{}\" ({at}).", c.repo_name)));
    }
    let repo = parsed.repo.unwrap_or_default();
    match found.project {
        Some(p) if p.repo_name != repo => {
            Err(Error::brf("WRONG_PROJECT", format!("This checkout is registered as \"{}\", not \"{repo}\".", p.repo_name)))
        }
        Some(_) => Ok(()),
        None => Err(Error::brf("NOT_REGISTERED", "This checkout is not in the registry.")),
    }
}

fn print(s: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{s}");
}

fn print_json(v: &Value) {
    print(&pretty(v));
}

/// A path under the home folder written as ~/...
fn tilde(path: &str, env: &Env) -> String {
    let home = env.home();
    match path.strip_prefix(&format!("{home}/")) {
        Some(rest) => format!("~/{rest}"),
        None => path.to_string(),
    }
}

fn run(argv: &[String]) -> Result<i32> {
    let cmd = argv.first().map_or("help", String::as_str);
    let args = parse_args(argv.get(1..).unwrap_or(&[]))?;
    let env = Env::process();
    let cfg = load_config(&env)?;
    let mut log = |s: &str| print(s);

    match cmd {
        "help" | "--help" | "-h" => {
            print(&help());
            Ok(0)
        }

        "version" | "--version" => {
            let t = current_templates(&env)?;
            let v = |i: usize| t[i].version.clone().unwrap_or_else(|| "null".into());
            print(&format!("brf {} · project template {} · meta template {}", brf_version(), v(0), v(1)));
            Ok(0)
        }

        "config" => {
            print_json(&cfg.to_json());
            Ok(0)
        }

        "init" => init(&args, &cfg, &env),

        "context" => {
            let date = today_stamp();
            let ctx = if args.on("meta") { meta_context(&cfg, &env, &date)? } else { project_context(&cfg, &env, &cwd(), &date)? };
            print_json(&ctx);
            Ok(0)
        }

        "register" => {
            let out_dir = require_output_dir(&cfg)?;
            let info = repo_info(&cwd())?;
            let today = today_stamp();
            let record = add_project(
                &out_dir,
                &NewEntry {
                    repo_name: to_repo_name(args.text("repo-name").unwrap_or(&info.repo_name)),
                    everyday_name: args.text("everyday-name").map(str::to_string),
                    local_path: Some(tilde(&info.main_path, &env)),
                    remote: info.remote.clone(),
                    added: Some(format!("{}-{}-{}", &today[0..4], &today[4..6], &today[6..8])),
                    reason: args.raw("reason").map(str::to_string),
                },
                &env,
            )?;
            print_json(&json!({ "registered": record, "registry": registry_path(&out_dir) }));
            Ok(0)
        }

        "snapshot" => {
            if let Some(file) = args.text("compare") {
                let r = compare_snapshot(&resolve(&[file]), &cwd())?;
                let pick = |v: &Value| {
                    let mut m = Map::new();
                    for k in ["head", "branch", "digest"] {
                        if let Some(x) = v.get(k) {
                            m.insert(k.into(), x.clone());
                        }
                    }
                    Value::Object(m)
                };
                print_json(&json!({ "same": r.same, "changed": r.changed, "before": pick(&r.before), "after": pick(&r.after) }));
                return Ok(if r.same { 0 } else { 1 });
            }
            let Some(out) = args.text("out") else {
                return Err(Error::brf("NO_OUT", "Pass --out <file> (keep it outside the repository) or --compare <file>."));
            };
            let here_dir = cwd();
            let snap = take_snapshot(&here_dir)?;
            let target = resolve(&[out]);
            let here = repo_info(&here_dir)?;
            let repo = snap["repo"].as_str().unwrap_or("").to_string();
            if [repo.as_str(), here.toplevel.as_str(), here.main_path.as_str()].iter().any(|root| is_inside(&target, root)) {
                return Err(Error::brf("INSIDE_REPO", "Write the snapshot outside the repository."));
            }
            save_snapshot(&target, &snap)?;
            print_json(&snap);
            Ok(0)
        }

        "build" => build_command(&args, &cfg, &env, &mut log),

        "deliver" => {
            let Some(input) = args.positional.first() else {
                return Err(Error::brf("NO_INPUT", "Pass the built brief: brf deliver <brf_repo_yyyymmdd.html>"));
            };
            let output_dir = require_output_dir(&cfg)?;
            let file = resolve(&[input]);
            checkout_guard(&file, &output_dir, &env)?;
            print_json(&deliver(&file, &output_dir, &cfg.cache_dir)?);
            Ok(0)
        }

        "check" => {
            let banned = cfg.banned_extra();
            if let Some(file) = args.text("file") {
                let path = resolve(&[file]);
                if !Path::new(&path).exists() {
                    return Err(Error::brf("NOT_FOUND", format!("Not found: {path}")));
                }
                let problems = check_file(&path, &banned)?;
                if args.on("json") {
                    print_json(&json!({ "problems": problems }));
                } else if problems.is_empty() {
                    print("CHECK CLEAN");
                } else {
                    print(&problems.iter().map(|p| format!("PROBLEM {p}")).collect::<Vec<_>>().join("\n"));
                }
                return Ok(if problems.is_empty() { 0 } else { 1 });
            }
            let out_dir = require_output_dir(&cfg)?;
            let r = check_folder_today(&out_dir, &banned)?;
            if args.on("json") {
                print_json(&r.to_json());
            } else {
                for p in &r.problems {
                    print(&format!("PROBLEM {p}"));
                }
                for n in &r.notes {
                    print(&format!("note    {n}"));
                }
                print(&format!("{} current project brief(s), {} registered project(s)", r.current_count, r.registered_count));
                print(if r.problems.is_empty() { "CHECK CLEAN" } else { "CHECK NOT CLEAN" });
            }
            Ok(if r.problems.is_empty() { 0 } else { 1 })
        }

        other => Err(Error::brf("UNKNOWN_COMMAND", format!("Unknown command \"{other}\". Run brf help."))),
    }
}

fn init(args: &Args, cfg: &Config, env: &Env) -> Result<i32> {
    let Some(output) = args.text("output") else {
        return Err(Error::brf("NO_OUTPUT", "Pass --output <folder>, the folder that should receive briefs."));
    };
    let out = resolve(&[&expand_home(output, env)]);
    let file = read_config_file(&cfg.config_path)?;
    if let Some(existing) = brf::config::file_str(&file, "output_dir") {
        if resolve(&[&expand_home(existing, env)]) != out && !args.on("force") {
            return Err(Error::brf("CONFIG_EXISTS", format!("This machine already points at {existing}. Pass --force to change it.")));
        }
    }
    std::fs::create_dir_all(&out)?;
    ensure_layout(&out)?;
    let reg = registry_path(&out);
    let created = !Path::new(&reg).exists();
    if created {
        write_json_atomic(&reg, &empty_registry())?;
    }
    let mut next = file.clone();
    next.insert("output_dir".into(), json!(tilde(&out, env)));
    if let Some(p) = args.text("playwright") {
        next.insert("playwright".into(), json!(p));
    }
    if let Some(u) = args.text("gh-user") {
        next.insert("gh_user".into(), json!(u));
    }
    if let Some(t) = args.text("templates-dir") {
        next.insert("templates_dir".into(), json!(t));
    }
    write_json_atomic(&cfg.config_path, &Value::Object(next))?;
    print_json(&json!({ "output_dir": out, "registry": reg, "registry_created": created, "config": cfg.config_path }));
    Ok(0)
}

fn build_command(args: &Args, cfg: &Config, env: &Env, log: &mut dyn FnMut(&str)) -> Result<i32> {
    let Some(input) = args.positional.first() else {
        return Err(Error::brf("NO_INPUT", "Pass the filled template: brf build <filled.html> --out <work_dir>/<name>"));
    };
    let Some(out_flag) = args.text("out") else {
        return Err(Error::brf("NO_OUT", "Pass --out with a path in your work folder, e.g. --out <work_dir>/brf_my-project_20260131.html"));
    };
    let brief = args.raw("brief").map(parse_number);
    let example = args.on("example");
    let output_dir = if example { None } else { Some(require_output_dir(cfg)?) };
    // Builds happen in the work folder. Never inside the repository being briefed, and never
    // straight into the output folder, where only brf deliver may add or move files.
    let out = resolve(&[out_flag]);
    let proof_dir = match args.text("proof-dir") {
        Some(p) => resolve(&[p]),
        None => resolve(&[&dirname(&out), "proof"]),
    };
    // Every checkout brf knows about: this one, its main checkout, and every registered project.
    let mut repo_roots: Vec<String> = Vec::new();
    if let Ok(here) = repo_info(&cwd()) {
        repo_roots.push(here.toplevel);
        repo_roots.push(here.main_path);
    }
    let registry_dir = output_dir.clone().or_else(|| cfg.output_dir.clone().filter(|d| Path::new(d).exists()));
    if let Some(dir) = &registry_dir {
        for p in load_registry(dir)?.projects {
            if let Some(lp) = p.local_path {
                repo_roots.push(expand_home(&lp, env));
            }
        }
    }
    for (what, path) in [("output", &out), ("proof folder", &proof_dir)] {
        if let Some(root) = repo_roots.iter().find(|root| Path::new(root.as_str()).exists() && is_inside(path, root)) {
            return Err(Error::brf(
                "INSIDE_REPO",
                format!("The {what} {path} is inside a repository ({root}). Build in the work folder from brf context."),
            ));
        }
        if let Some(od) = cfg.output_dir.as_deref().filter(|d| Path::new(d).exists()) {
            if is_inside(path, od) {
                return Err(Error::brf("INSIDE_OUTPUT", format!("The {what} {path} is inside the output folder. Build in the work folder, then brf deliver.")));
            }
        }
    }
    if !example {
        checkout_guard(&out, output_dir.as_deref().unwrap_or(""), env)?;
    }
    let built = build(&BuildOptions { input, out: &out, brief, example, output_dir: output_dir.as_deref(), env }, log)?;
    if !args.on("verify") {
        log("(pass --verify to render it at five widths)");
        return Ok(0);
    }
    let _ = std::io::stdout().flush();
    Ok(verify(&built.path, &proof_dir, cfg, log)?.exit_code())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&argv) {
        Ok(code) => code,
        Err(Error::Brf { code, message }) => {
            eprintln!("brf: {message} ({code})");
            2
        }
        Err(Error::Other(message)) => {
            eprintln!("brf: {message}");
            1
        }
    };
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}
