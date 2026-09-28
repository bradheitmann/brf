// Ported from test/repo.test.mjs.
mod common;
use brf::repo::{redact_remote, repo_info};
use brf::snapshot::{compare_snapshot, save_snapshot, take_snapshot};
use common::*;

// Fake credentials, joined at run time so the fixture is not mistaken for an address.
const AT: &str = "@";

#[test]
fn names_the_repository_from_its_remote_and_strips_credentials() {
    let tmp = temp_dir();
    let dir = git_repo(&tmp.join("local-folder"), Some(&format!("https://someone:secret{AT}github.com/acme/Tide_Water.git")));
    let info = repo_info(&dir).unwrap();
    assert_eq!(info.repo_name, "tide-water");
    assert_eq!(info.remote.as_deref(), Some("https://github.com/acme/Tide_Water.git"));
    let ssh = format!("git{AT}github.com:acme/x.git");
    assert_eq!(redact_remote(&ssh), ssh);
}

#[test]
fn falls_back_to_the_folder_name_without_a_remote() {
    let tmp = temp_dir();
    let dir = git_repo(&tmp.join("My_Project"), None);
    assert_eq!(repo_info(&dir).unwrap().repo_name, "my-project");
}

#[test]
fn a_linked_worktree_briefs_the_main_project() {
    let tmp = temp_dir();
    let main = git_repo(&tmp.join("harbor-log"), None);
    let tmp2 = temp_dir();
    let wt = tmp2.join("harbor-log-feature");
    git(&main, &["worktree", "add", "-q", "-b", "feature", &wt]);
    let info = repo_info(&wt).unwrap();
    assert!(info.is_linked_worktree);
    assert_eq!(info.repo_name, "harbor-log");
}

#[test]
fn snapshots_match_when_nothing_changed_and_differ_when_something_did() {
    let tmp = temp_dir();
    let dir = git_repo(&tmp.join("snap"), None);
    let files = temp_dir();
    let file = files.join("before.json");
    save_snapshot(&file, &take_snapshot(&dir).unwrap()).unwrap();
    assert!(compare_snapshot(&file, &dir).unwrap().same);
    write(&format!("{dir}/new.txt"), "x");
    assert!(!compare_snapshot(&file, &dir).unwrap().same);
}
