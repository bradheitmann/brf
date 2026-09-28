// Ported from test/registry.test.mjs.
mod common;
use brf::registry::{Checkout, NewEntry, add_project, find_project, load_registry, match_checkout, remote_key};
use common::*;
use serde_json::json;

// Fake SSH remotes, joined at run time so they are not mistaken for addresses.
const AT: &str = "@";

fn entry(name: &str, reason: &str) -> NewEntry {
    NewEntry { repo_name: name.into(), reason: Some(reason.into()), ..Default::default() }
}

fn checkout(name: &str, main: Option<&str>, remote: Option<&str>) -> Checkout {
    Checkout { repo_name: name.into(), main_path: main.map(str::to_string), remote: remote.map(str::to_string) }
}

#[test]
fn adds_a_project_only_with_the_owner_s_words_once() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    write_registry(out.path(), json!([]));
    assert!(err_text(add_project(out.path(), &entry("tidewater", ""), &env)).contains("owner asks"));
    let first =
        NewEntry { everyday_name: Some("Tidewater".into()), added: Some("2026-11-02".into()), ..entry("tidewater", "Brief Tidewater weekly") };
    add_project(out.path(), &first, &env).unwrap();
    assert!(err_text(add_project(out.path(), &entry("tidewater", "again please"), &env)).contains("already registered"));
    assert!(err_text(add_project(out.path(), &entry("Bad_Name", "please add"), &env)).contains("lower-case"));
    let reg = load_registry(out.path()).unwrap();
    assert_eq!(reg.projects.len(), 1);
    let raw: serde_json::Value = serde_json::from_str(&read(&reg.path)).unwrap();
    assert_eq!(raw["projects"][0]["reason"], "Brief Tidewater weekly");
}

#[test]
fn matches_a_checkout_by_its_recorded_path_or_remote_never_by_name_alone() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    let repo = out.join("checkout");
    let elsewhere = out.join("elsewhere");
    std::fs::create_dir(&repo).unwrap();
    std::fs::create_dir(&elsewhere).unwrap();
    write_registry(
        out.path(),
        json!([
            { "repo_name": "pinned-name", "local_path": repo },
            { "repo_name": "tidewater", "remote": format!("git{AT}github.com:acme/tidewater.git") },
            { "repo_name": "bare-name" },
        ]),
    );
    let reg = load_registry(out.path()).unwrap();
    assert_eq!(find_project(&reg, "pinned-name").unwrap().repo_name, "pinned-name");
    let name = |m: brf::registry::Match| (m.project.map(|p| p.repo_name), m.conflict.map(|p| p.repo_name));
    assert_eq!(name(match_checkout(&reg, &checkout("other", Some(&repo), None), &env)).0.as_deref(), Some("pinned-name"));
    assert_eq!(name(match_checkout(&reg, &checkout("pinned-name", Some(&elsewhere), None), &env)).1.as_deref(), Some("pinned-name"));
    assert_eq!(
        name(match_checkout(&reg, &checkout("tidewater", Some(&elsewhere), Some("https://github.com/ACME/tidewater")), &env)).0.as_deref(),
        Some("tidewater")
    );
    let other = name(match_checkout(&reg, &checkout("tidewater", Some(&elsewhere), Some("https://github.com/someone-else/tidewater.git")), &env));
    assert_eq!(other.0, None);
    assert_eq!(other.1.as_deref(), Some("tidewater"));
    assert_eq!(name(match_checkout(&reg, &checkout("bare-name", Some(&elsewhere), None), &env)).0.as_deref(), Some("bare-name"));
    assert_eq!(name(match_checkout(&reg, &checkout("nobody", Some(&elsewhere), None), &env)).0, None);
}

#[test]
fn remote_keys_ignore_scheme_host_alias_case_and_git() {
    for u in [
        format!("git{AT}github.com:Acme/Tide.git"),
        "https://github.com/acme/tide".to_string(),
        format!("ssh://git{AT}github.com/acme/tide.git"),
        format!("git{AT}github-work:acme/tide.git"),
    ] {
        assert_eq!(remote_key(&u).as_deref(), Some("acme/tide"), "{u}");
    }
}

#[test]
fn refuses_to_register_the_same_checkout_or_remote_twice_under_another_name() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    write_registry(out.path(), json!([]));
    let harbor = out.join("harbor");
    add_project(
        out.path(),
        &NewEntry {
            local_path: Some(harbor.clone()),
            remote: Some(format!("git{AT}github.com:acme/harbor-log.git")),
            ..entry("harbor-log", "add harbor")
        },
        &env,
    )
    .unwrap();
    assert!(
        err_text(add_project(out.path(), &NewEntry { local_path: Some(harbor.clone()), ..entry("harbor-local", "again") }, &env))
            .contains("already registered as \"harbor-log\"")
    );
    assert!(
        err_text(add_project(
            out.path(),
            &NewEntry { remote: Some("https://github.com/acme/harbor-log".into()), ..entry("harbor-2", "again") },
            &env
        ))
        .contains("already registered")
    );
}

#[test]
fn a_missing_registry_is_empty_a_broken_one_is_an_error() {
    let out = temp_dir();
    assert!(!load_registry(out.path()).unwrap().exists);
    brf::config::write_json_atomic(&brf::registry::registry_path(out.path()), &json!({ "projects": [{ "repo_name": "NOPE" }] })).unwrap();
    assert!(err_text(load_registry(out.path())).contains("lower-case-hyphenated"));
}
