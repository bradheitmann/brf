// Ported from test/cli.test.mjs.
mod common;
use common::*;

#[test]
fn prints_versions() {
    let home = temp_dir();
    let r = cli(&["version"], ROOT, home.path());
    assert_eq!(r.code, 0);
    assert!(re(r"^brf \d+\.\d+\.\d+ · project template \d+\.\d+\.\d+ · meta template \d+\.\d+\.\d+").is_match(&r.stdout), "{}", r.stdout);
}

#[test]
fn context_no_folder_then_unregistered_then_registered() {
    let home = temp_dir();
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("tidewater"), None);
    let r = cli(&["context"], &repo, home.path());
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("No output folder is set"));

    let outs = temp_dir();
    let out = outs.join("daily_brf");
    assert_eq!(cli(&["init", "--output", &out], &repo, home.path()).code, 0);
    let r = cli(&["context"], &repo, home.path());
    assert_eq!(r.code, 0);
    assert!(r.json()["stop"].as_str().unwrap().contains("not in the registry"));

    assert_eq!(cli(&["register", "--reason", ""], &repo, home.path()).code, 2);
    assert_eq!(cli(&["register", "--reason", "Please brief Tidewater", "--everyday-name", "Tidewater"], &repo, home.path()).code, 0);
    let ctx = cli(&["context"], &repo, home.path()).json();
    assert_eq!(ctx["registered"], true);
    assert_eq!(ctx["brief_number"], 1);
    assert!(re(r"^brf_tidewater_\d{8}\.html$").is_match(ctx["output_name"].as_str().unwrap()));
    assert!(!ctx["work_dir"].as_str().unwrap().starts_with(&repo));
    let reg: serde_json::Value = serde_json::from_str(&read(&format!("{out}/registry.json"))).unwrap();
    assert_eq!(reg["projects"][0]["reason"], "Please brief Tidewater");
}

#[test]
fn init_refuses_to_repoint_a_configured_computer_without_force() {
    let home = temp_dir();
    let (a, b) = (temp_dir(), temp_dir());
    assert_eq!(cli(&["init", "--output", &a.join("a")], ROOT, home.path()).code, 0);
    assert_eq!(cli(&["init", "--output", &b.join("b")], ROOT, home.path()).code, 2);
    assert_eq!(cli(&["init", "--output", &b.join("b"), "--force"], ROOT, home.path()).code, 0);
}

#[test]
fn snapshot_refuses_to_write_inside_the_repository() {
    let home = temp_dir();
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("snapper"), None);
    let r = cli(&["snapshot", "--out", &format!("{repo}/before.json")], &repo, home.path());
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("outside the repository"));
}
