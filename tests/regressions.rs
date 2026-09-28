// Ported from test/regressions.test.mjs: one test per defect found in the first independent review.
mod common;
use brf::build::BuildOptions;
use brf::check::{check_folder, check_html};
use brf::context::mister_clean;
use brf::deliver::{LOCK_STALE, LOCK_WAIT, deliver, with_lock};
use brf::folder::{ensure_layout, next_number};
use brf::naming::META_KEY;
use brf::registry::{Checkout, load_registry, match_checkout, remote_key};
use brf::snapshot::{compare_snapshot, save_snapshot, take_snapshot};
use common::*;
use serde_json::json;
use std::path::Path;
use std::time::Duration;

const NAME: &str = "brf_tidewater_20261102.html";

fn g(cwd: &str, args: &[&str]) {
    git(cwd, args);
}

struct Registered {
    home: TempDir,
    _repo_parent: TempDir,
    repo: String,
    _out_parent: TempDir,
    out: String,
}

fn registered(name: &str) -> Registered {
    let home = temp_dir();
    let repo_parent = temp_dir();
    let repo = git_repo(&repo_parent.join(name), None);
    let out_parent = temp_dir();
    let out = out_parent.join("daily_brf");
    assert_eq!(cli(&["init", "--output", &out], &repo, home.path()).code, 0);
    assert_eq!(cli(&["register", "--reason", "Please brief it"], &repo, home.path()).code, 0);
    Registered { home, _repo_parent: repo_parent, repo, _out_parent: out_parent, out }
}

fn example_html() -> String {
    let home = temp_dir();
    built_example(&test_env(home.path()), None)
}

fn with_snippet(html: &str, snippet: &str) -> String {
    html.replacen("</main>", &format!("{snippet}</main>"), 1)
}

fn caught(html: &str, snippet: &str) -> bool {
    !check_html(&with_snippet(html, snippet), NAME, &[]).is_empty()
}

fn clean(html: &str, snippet: &str) -> Vec<String> {
    check_html(&with_snippet(html, snippet), NAME, &[])
}

#[test]
fn build_refuses_an_output_or_proof_folder_inside_the_repository() {
    let r = registered("tidewater");
    let status = git_out(&r.repo, &["status", "--porcelain", "--ignored"]);
    let ex = example();
    let run = cli(&["build", &ex, "--out", NAME], &r.repo, r.home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("inside a repository"), "{}", run.stderr);
    let tmp = temp_dir();
    let run = cli(&["build", &ex, "--out", &tmp.join(NAME), "--proof-dir", &format!("{}/proof", r.repo)], &r.repo, r.home.path());
    assert_eq!(run.code, 2);
    assert_eq!(git_out(&r.repo, &["status", "--porcelain", "--ignored"]), status);
}

#[test]
fn build_refuses_to_write_into_the_output_folder() {
    let r = registered("tidewater");
    let run = cli(&["build", &example(), "--out", &format!("{}/{NAME}", r.out)], &r.repo, r.home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("inside the output folder"), "{}", run.stderr);
}

#[test]
fn an_unregistered_repository_with_a_registered_name_is_stopped() {
    let r = registered("tidewater");
    let tmp = temp_dir();
    let impostor = git_repo(&tmp.join("tidewater"), Some("https://github.com/someone-else/tidewater.git"));
    let ctx = cli(&["context"], &impostor, r.home.path()).json();
    assert_eq!(ctx["registered"], false);
    assert!(ctx["stop"].as_str().unwrap().contains("different checkout"));
}

#[test]
fn a_project_called_meta_does_not_share_numbers_or_filing_with_the_meta_brief() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "meta" }]));
    write(&out.join("meta_brf_20260920.html"), r#"<meta name="brf-brief" content="1">"#);
    assert_eq!(next_number(out.path(), "meta"), 1);
    assert_eq!(next_number(out.path(), META_KEY), 2);
    let work = temp_dir();
    let target = work.join("brf_meta_20261102.html");
    let ex = example();
    let r = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &target, &env) }).unwrap();
    assert_eq!(r.brief, 1);
    let cache = temp_dir();
    let d = deliver(&r.path, out.path(), cache.path()).unwrap();
    assert_eq!(d["archived"], json!([]));
    assert_eq!(read(&out.join("meta_brf_20260920.html")), r#"<meta name="brf-brief" content="1">"#);
}

#[test]
fn context_output_survives_a_pipe_at_any_size() {
    let r = registered("tidewater");
    for i in 0..600 {
        let (y, m, d) = brf::util::civil_from_days(brf::util::days_from_civil(2024, 1, 1) + i);
        write(&format!("{}/archive/brf_tidewater_{y:04}{m:02}{d:02}.html", r.out), "<p>old</p>");
    }
    let run = cli(&["context"], &r.repo, r.home.path());
    assert_eq!(run.code, 0);
    assert!(run.stdout.len() > 70000);
    assert_eq!(run.json()["brief_number"], 601);
}

#[test]
fn the_snapshot_sees_edits_branch_switches_and_stash_round_trips() {
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("snap"), None);
    write(&format!("{repo}/README.md"), "edited once\n");
    let files = temp_dir();
    let file = files.join("before.json");
    let fresh = || save_snapshot(&file, &take_snapshot(&repo).unwrap()).unwrap();
    let same = || compare_snapshot(&file, &repo).unwrap().same;
    fresh();
    write(&format!("{repo}/README.md"), "edited twice\n");
    assert!(!same(), "further edit to a modified file");
    fresh();
    g(&repo, &["switch", "-q", "-c", "other"]);
    assert!(!same(), "branch switch at the same commit");
    g(&repo, &["switch", "-q", "-"]);
    fresh();
    g(&repo, &["stash", "-q"]);
    g(&repo, &["stash", "apply", "-q"]);
    assert!(!same(), "stash then apply");
    fresh();
    g(&repo, &["stash", "-q"]);
    g(&repo, &["stash", "pop", "-q"]);
    assert!(!same(), "stash then pop");
    fresh();
    g(&repo, &["switch", "-q", "-c", "away"]);
    g(&repo, &["switch", "-q", "-"]);
    assert!(!same(), "switch away and back");
    fresh();
    write(&format!("{repo}/untracked.txt"), "a");
    fresh();
    write(&format!("{repo}/untracked.txt"), "b");
    assert!(!same(), "untracked content change");
    fresh();
    let tmp2 = temp_dir();
    let other = git_repo(&tmp2.join("other"), None);
    assert!(err_text(compare_snapshot(&file, &other)).contains("taken in"));
}

#[test]
fn mister_clean_is_found_on_path_and_its_version_comes_only_from_its_own_package() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp_dir();
    let pkg = root.join("global/node_modules/@bradheitmann/mister-clean");
    std::fs::create_dir_all(format!("{pkg}/bin")).unwrap();
    write(&format!("{pkg}/package.json"), &json!({ "name": "@bradheitmann/mister-clean", "version": "9.9.9" }).to_string());
    write(&format!("{pkg}/bin/mister-clean.js"), "console.log('usage')\n");
    // A package-manager shim, next to an unrelated package.json that must be ignored.
    let shims = root.join("shims");
    std::fs::create_dir(&shims).unwrap();
    write(&root.join("package.json"), &json!({ "name": "someone-else", "version": "1.0.0" }).to_string());
    let shim = format!("{shims}/mister-clean");
    write(&shim, &format!("#!/bin/sh\nexec node \"$basedir/x.js\" \"$@\"\n# cmd-shim-target={pkg}/bin/mister-clean.js\n"));
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(mister_clean(&shims), json!({ "path": shim, "version": "9.9.9" }));
    let plain = root.join("plain");
    std::fs::create_dir(&plain).unwrap();
    let plain_bin = format!("{plain}/mister-clean");
    write(&plain_bin, "#!/bin/sh\necho usage; exit 2\n");
    std::fs::set_permissions(&plain_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(mister_clean(&plain), json!({ "path": plain_bin, "version": null }));
    let empty = temp_dir();
    assert_eq!(mister_clean(empty.path()), serde_json::Value::Null);
}

#[test]
fn check_catches_every_way_a_page_can_run_code_or_reach_the_network() {
    let html = example_html();
    for (what, snippet) in [
        ("svg image", r#"<svg><image href="https://example.com/a.png"/></svg>"#),
        ("srcset", r#"<img srcset="https://example.com/a.png 2x">"#),
        ("meta refresh", r#"<meta http-equiv="refresh" content="0;url=https://example.com">"#),
        ("event handler", r#"<img src="x.png" onerror="fetch(1)">"#),
        ("javascript link", r#"<a href="javascript:alert(1)">x</a>"#),
        ("iframe", "<iframe></iframe>"),
        ("protocol-relative", r#"<link rel="stylesheet" href="//example.com/a.css">"#),
    ] {
        assert!(caught(&html, snippet), "{what} was not caught");
    }
    assert_eq!(clean(&html, r#"<a href="https://example.com">a link</a>"#), Vec::<String>::new());
}

#[test]
fn a_stamp_that_disagrees_with_the_file_name_is_refused() {
    let html = example_html();
    assert!(check_html(&html, "meta_brf_20261102.html", &[]).join("\n").contains("project brief but named as a meta brief"));
    assert!(check_html(&html, "brf_tidewater_20261103.html", &[]).join("\n").contains("stamped 2026-11-02 but named 2026-11-03"));
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([]));
    let dir = temp_dir();
    let renamed = dir.join("meta_brf_20261102.html");
    write(&renamed, &html);
    let cache = temp_dir();
    assert!(err_text(deliver(&renamed, out.path(), cache.path())).contains("named as a meta brief"));
}

#[test]
fn the_whole_flow_leaves_the_repository_exactly_as_it_was() {
    let r = registered("tidewater");
    let ctx = cli(&["context"], &r.repo, r.home.path()).json();
    let work = ctx["work_dir"].as_str().unwrap();
    let before = format!("{work}/before.json");
    assert_eq!(cli(&["snapshot", "--out", &before], &r.repo, r.home.path()).code, 0);
    let built = format!("{work}/{}", ctx["output_name"].as_str().unwrap());
    assert_eq!(cli(&["build", &example(), "--out", &built], &r.repo, r.home.path()).code, 0);
    assert_eq!(cli(&["check", "--file", &built], &r.repo, r.home.path()).code, 0);
    assert_eq!(cli(&["snapshot", "--compare", &before], &r.repo, r.home.path()).json()["same"], true);
    assert_eq!(cli(&["deliver", &built], &r.repo, r.home.path()).json()["brief"], 1);
    assert_eq!(cli(&["snapshot", "--compare", &before], &r.repo, r.home.path()).json()["same"], true);
    assert!(Path::new(&r.out).exists());
}

#[test]
fn another_clone_of_a_project_registered_under_its_own_name_is_recognised() {
    let home = temp_dir();
    let remote = "https://github.com/acme/Harbor_Svc.git";
    let t1 = temp_dir();
    let first = git_repo(&t1.join("harbor"), Some(remote));
    let t2 = temp_dir();
    let out = t2.join("daily_brf");
    assert_eq!(cli(&["init", "--output", &out], &first, home.path()).code, 0);
    assert_eq!(cli(&["register", "--reason", "Add Harbor Log to my briefs", "--repo-name", "harbor-log"], &first, home.path()).code, 0);
    let t3 = temp_dir();
    let second = git_repo(&t3.join("harbor-copy"), Some(remote));
    let ctx = cli(&["context"], &second, home.path()).json();
    assert_eq!(ctx["registered"], true);
    assert_eq!(ctx["repo"]["repo_name"], "harbor-log");
}

#[test]
fn build_and_deliver_refuse_a_different_checkout_that_shares_a_registered_name() {
    let r = registered("tidewater");
    let tmp = temp_dir();
    let impostor = git_repo(&tmp.join("tidewater"), Some("https://github.com/someone-else/tidewater.git"));
    let t2 = temp_dir();
    let target = t2.join(NAME);
    let run = cli(&["build", &example(), "--out", &target], &impostor, r.home.path());
    assert_eq!(run.code, 2);
    assert!(re(r#"not the registered "tidewater".*\(OTHER_CHECKOUT\)"#).is_match(&run.stderr), "{}", run.stderr);
    let env = test_env(r.home.path());
    let ex = example();
    let made = build_quiet(&BuildOptions { output_dir: Some(&r.out), ..opts(&ex, &target, &env) }).unwrap();
    let d = cli(&["deliver", &made.path], &impostor, r.home.path());
    assert_eq!(d.code, 2);
    assert!(d.stderr.contains("OTHER_CHECKOUT"));
}

#[test]
fn snapshot_compare_says_what_changed() {
    let home = temp_dir();
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("compare"), None);
    let files = temp_dir();
    let before = files.join("before.json");
    assert_eq!(cli(&["snapshot", "--out", &before], &repo, home.path()).code, 0);
    g(&repo, &["switch", "-q", "-c", "elsewhere"]);
    let run = cli(&["snapshot", "--compare", &before], &repo, home.path());
    assert_eq!(run.code, 1);
    let out = run.json();
    assert_eq!(out["same"], false);
    assert!(out["changed"].as_array().unwrap().contains(&json!("branch")));
    assert_ne!(out["before"]["branch"], out["after"]["branch"]);
}

#[test]
fn check_catches_event_handlers_written_without_spaces_or_after_a_quoted_gt() {
    let html = example_html();
    for snippet in [
        r#"<img/src="x.png"/onerror="alert(1)">"#,
        r#"<svg/onload="alert(1)"></svg>"#,
        r#"<img src="x.png"onerror="alert(1)">"#,
        r#"<img alt="a>b" src="x.png" onerror="alert(1)">"#,
        r#"<img alt="a>b" src="https://example.com/a.png">"#,
        "<img src=x alt=it's onerror=alert(1)>",
        r#"<img src=x title=a"b onerror=alert(1)//">"#,
        "<img alt=it's src=https://example.com/a.png>",
        r#"<a href="&#106;avascript:alert(1)">x</a>"#,
        r#"<img src="https:&#47;&#47;example.com/a.png">"#,
        "<a href=\"java\tscript:alert(1)\">x</a>",
    ] {
        assert!(caught(&html, snippet), "{snippet}");
    }
    assert_eq!(clean(&html, r#"<p title="turn online=yes">text</p>"#), Vec::<String>::new());
    assert_eq!(clean(&html, r#"<p title="set src=https://example.com">text</p>"#), Vec::<String>::new());
}

#[test]
fn remote_keys_keep_the_whole_path_so_unrelated_repos_with_the_same_last_two_parts_differ() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    write_registry(out.path(), json!([{ "repo_name": "api", "remote": "https://dev.azure.com/orgA/Payments/_git/api" }]));
    let reg = load_registry(out.path()).unwrap();
    let t = temp_dir();
    let co = |remote: &str| Checkout { repo_name: "api".into(), main_path: Some(t.path().to_string()), remote: Some(remote.into()) };
    assert!(match_checkout(&reg, &co("https://dev.azure.com/orgB/Unrelated/_git/api"), &env).project.is_none());
    let with_user = format!("https://user{}dev.azure.com/orgA/Payments/_git/api/", "@");
    assert_eq!(match_checkout(&reg, &co(&with_user), &env).project.unwrap().repo_name, "api");
    assert_ne!(remote_key("https://gitlab.com/team-a/platform/api.git"), remote_key("https://gitlab.com/team-b/platform/api.git"));
}

#[test]
fn the_snapshot_ignores_signature_display_even_on_signed_commits() {
    if std::process::Command::new("ssh-keygen").arg("-V").output().is_err() {
        eprintln!("skipped: ssh-keygen not available");
        return;
    }
    let keys = temp_dir();
    let key = keys.join("id");
    let st = std::process::Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f", &key]).status().unwrap();
    assert!(st.success());
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("signed"), None);
    g(&repo, &["config", "gpg.format", "ssh"]);
    g(&repo, &["config", "user.signingkey", &key]);
    write(&format!("{repo}/signed.txt"), "x");
    g(&repo, &["add", "signed.txt"]);
    let st = std::process::Command::new("git")
        .args(["-c", "user.email=brf-test", "-c", "user.name=t", "-c", "commit.gpgsign=true", "commit", "-q", "-m", "signed"])
        .current_dir(&repo)
        .status()
        .unwrap();
    assert!(st.success());
    g(&repo, &["config", "log.showSignature", "true"]);
    let files = temp_dir();
    let file = files.join("before.json");
    save_snapshot(&file, &take_snapshot(&repo).unwrap()).unwrap();
    // Trusting the key changes what signature verification prints; the repository is unchanged.
    write(&keys.join("allowed"), &format!("brf-test {}", read(&format!("{key}.pub"))));
    g(&repo, &["config", "gpg.ssh.allowedSignersFile", &keys.join("allowed")]);
    let r = compare_snapshot(&file, &repo).unwrap();
    assert!(r.same, "changed: {:?}", r.changed);
}

#[test]
fn check_follows_html_s_rules_for_comments_text_only_elements_whitespace_and_entities() {
    let html = example_html();
    let nbsp = "\u{a0}";
    for snippet in [
        "<!--> <img src=x onerror=alert(1)> -->".to_string(),
        "<!---> <img src=x onerror=alert(1)> -->".into(),
        "<!-- a --!> <img src=x onerror=alert(1)> -->".into(),
        r#"<img alt="<!--" src=x onerror=alert(1)><p title="-->">"#.into(),
        r#"<style>/* <a title=" */</style><img src=x onerror=alert(1)><style>/* " */</style>"#.into(),
        r#"<!x <a title=" ><img src=x onerror=alert(1)><b title=x">"#.into(),
        format!(r#"<p title={nbsp}"a onerror=alert(1) b=x">x</p>"#),
        r#"<p class="https&colon;//example.com">x</p><img src="https&colon;//example.com/a.png">"#.into(),
        r#"<a href="https&colon;//example.com">x</a>"#.into(),
        r#"<link rel=preload as=image imagesrcset="https://example.com/a.png">"#.into(),
        r"<style>body{background:url(\68ttps://example.com/a.png)}</style>".into(),
        "<style>body{background:url(https:example.com/a.png)}</style>".into(),
        "<svg><style>rect{fill:url(&#104;ttps://example.com/a)}</style></svg>".into(),
        r#"<p style="background:url(https://example.com/a.png)">x</p>"#.into(),
        r#"<meta http-equiv="refresh" content="0;url=https://example.com">"#.into(),
    ] {
        assert!(caught(&html, &snippet), "{snippet}");
    }
    let at = "@";
    for ordinary in [
        "<p>The fonts came in with @import once; now they are embedded.</p>".to_string(),
        format!("<p>Write to billing{at}importfreight.example for invoices.</p>"),
        "<p>If a &lt; b and x&lt;y, nothing happens &#9999999; at all.</p>".into(),
        r#"<p title="set src=https://example.com">x</p>"#.into(),
        r#"<a href="https://example.com/?a=1&amp;b=2">a link</a>"#.into(),
        r#"<a href="&#104ttps://example.com/&#9999999;">a link written with odd entities</a>"#.into(),
    ] {
        assert_eq!(clean(&html, &ordinary), Vec::<String>::new(), "{ordinary}");
    }
}

#[test]
fn remote_keys_agree_across_https_and_ssh_on_azure_devops_and_bitbucket_server() {
    let at = "@";
    assert_eq!(remote_key("https://dev.azure.com/OrgA/Payments/_git/api"), remote_key(&format!("git{at}ssh.dev.azure.com:v3/OrgA/Payments/api")));
    assert_eq!(
        remote_key("https://bitbucket.example.com/scm/proj/repo.git"),
        remote_key(&format!("ssh://git{at}bitbucket.example.com:7999/proj/repo.git"))
    );
    assert_eq!(remote_key("https://github.com/scm/repo").as_deref(), Some("scm/repo"));
}

#[test]
fn css_comment_markers_inside_strings_cannot_hide_a_fetch() {
    let html = example_html();
    for css in [
        r#".a{content:"/*"}body{background:url(https://example.com/a.png)}.b{content:"*/"}"#,
        r".a{b:\/*}body{background:url(https://example.com/a.png)}.c{d:*/}",
    ] {
        let problems = clean(&html, &format!("<style>{css}</style>")).join("\n");
        assert!(problems.contains("fetches from the network"), "{css}");
    }
}

#[test]
fn build_and_deliver_refuse_a_page_with_a_script_whether_or_not_check_was_run() {
    let home = temp_dir();
    let env = test_env(home.path());
    let dir = temp_dir();
    let filled = dir.join("bad.filled.html");
    write(&filled, &read(&example()).replacen("</main>", r#"<script>fetch("https://example.com/x")</script></main>"#, 1));
    let target = dir.join(NAME);
    let e = err_text(build_quiet(&BuildOptions { example: true, ..opts(&filled, &target, &env) }));
    assert!(e.contains("UNSAFE") || e.contains("script element"), "{e}");
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }]));
    let ex = example();
    let later = dir.join("brf_tidewater_20261103.html");
    let good = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &later, &env) }).unwrap();
    write(&good.path, &read(&good.path).replacen("</main>", "<script>1</script></main>", 1));
    let cache = temp_dir();
    assert!(err_text(deliver(&good.path, out.path(), cache.path())).contains("script element"));
}

#[test]
fn build_refuses_a_registered_project_s_folder_from_anywhere_and_another_spelling_of_it() {
    let r = registered("tidewater");
    let elsewhere = temp_dir();
    let ex = example();
    let run = cli(&["build", &ex, "--out", &format!("{}/{NAME}", r.repo)], elsewhere.path(), r.home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("inside a repository"), "{}", run.stderr);
    let upper = format!("{}TIDEWATER", r.repo.strip_suffix("tidewater").unwrap());
    if Path::new(&upper).exists() {
        let run = cli(&["build", &ex, "--out", &format!("{upper}/{NAME}")], &r.repo, r.home.path());
        assert_eq!(run.code, 2, "a differently cased path to the repository");
    }
    let t = temp_dir();
    let run = cli(&["build", &ex, &format!("--out={}", t.join(NAME))], &r.repo, r.home.path());
    assert_eq!(run.code, 0, "--flag=value works: {}", run.stderr);
}

#[test]
fn a_missing_output_folder_is_the_owner_s_to_fix() {
    let r = registered("tidewater");
    std::fs::rename(&r.out, format!("{}-away", r.out)).unwrap();
    let run = cli(&["context"], &r.repo, r.home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("Tell the owner; only the owner runs brf init"), "{}", run.stderr);
    assert!(!Path::new(&r.out).exists());
}

#[test]
fn the_same_folder_with_a_different_remote_is_a_different_project() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    let folder = temp_dir();
    write_registry(out.path(), json!([{ "repo_name": "api", "local_path": folder.path(), "remote": "https://github.com/acme/api.git" }]));
    let reg = load_registry(out.path()).unwrap();
    let r = match_checkout(
        &reg,
        &Checkout {
            repo_name: "website".into(),
            main_path: Some(folder.path().into()),
            remote: Some("https://github.com/someone-else/website.git".into()),
        },
        &env,
    );
    assert!(r.project.is_none());
    assert_eq!(r.conflict.unwrap().repo_name, "api");
    let r = match_checkout(&reg, &Checkout { repo_name: "x".into(), main_path: Some(folder.path().into()), remote: None }, &env);
    assert_eq!(r.project.unwrap().repo_name, "api");
}

#[test]
fn deliver_checks_the_registry_itself() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }]));
    let work = temp_dir();
    let target = work.join(NAME);
    let ex = example();
    let r = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &target, &env) }).unwrap();
    write_registry(out.path(), json!([]));
    let cache = temp_dir();
    assert!(err_text(deliver(&r.path, out.path(), cache.path())).contains("not in the registry"));
}

#[test]
fn init_on_a_second_computer_keeps_the_existing_registry() {
    let r = registered("tidewater");
    let before = read(&format!("{}/registry.json", r.out));
    let other = temp_dir();
    let other_home = temp_dir();
    assert_eq!(cli(&["init", "--output", &r.out], other.path(), other_home.path()).code, 0);
    assert_eq!(read(&format!("{}/registry.json", r.out)), before);
}

#[test]
fn check_notices_a_current_brief_whose_number_is_behind_the_archive() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }]));
    let work = temp_dir();
    let target = work.join(NAME);
    let ex = example();
    let r = build_quiet(&BuildOptions { output_dir: Some(out.path()), brief: Some(1.0), ..opts(&ex, &target, &env) }).unwrap();
    std::fs::copy(&r.path, out.join(NAME)).unwrap();
    write(&out.join("archive/brf_tidewater_20261001.html"), r#"<meta name="brf-brief" content="5">"#);
    let result = check_folder(out.path(), &[], 14, "20261102").unwrap();
    assert!(result.problems.join("\n").contains("is brief 1 but the archive holds brief 5"));
}

#[test]
fn the_snapshot_sees_a_change_that_is_both_staged_and_in_the_working_file() {
    let tmp = temp_dir();
    let repo = git_repo(&tmp.join("staged"), None);
    let files = temp_dir();
    let file = files.join("before.json");
    save_snapshot(&file, &take_snapshot(&repo).unwrap()).unwrap();
    write(&format!("{repo}/README.md"), "changed and staged\n");
    g(&repo, &["add", "README.md"]);
    let r = compare_snapshot(&file, &repo).unwrap();
    assert!(!r.same);
    assert!(r.changed.contains(&"staged".to_string()));
}

#[test]
fn deliveries_take_turns_through_the_lock_and_a_stale_lock_is_broken() {
    let cache = temp_dir();
    let out = temp_dir();
    let short = Duration::from_millis(300);
    let mut inner: Option<String> = None;
    with_lock(cache.path(), out.path(), LOCK_WAIT, LOCK_STALE, || {
        inner = Some(match with_lock(cache.path(), out.path(), short, LOCK_STALE, || Ok(1)) {
            Ok(_) => "entered".into(),
            Err(e) => e.code().unwrap_or("").to_string(),
        });
        Ok(())
    })
    .unwrap();
    assert_eq!(inner.as_deref(), Some("LOCKED"));
    assert_eq!(with_lock(cache.path(), out.path(), LOCK_WAIT, LOCK_STALE, || Ok("free again")).unwrap(), "free again");
    // A lock left behind by a crashed process is taken over once it is old enough.
    with_lock(cache.path(), out.path(), LOCK_WAIT, LOCK_STALE, || {
        let locks = cache.join("locks");
        for f in brf::folder::sorted_names(&locks) {
            let p = std::ffi::CString::new(format!("{locks}/{f}")).unwrap();
            let times = [libc::timeval { tv_sec: 0, tv_usec: 0 }, libc::timeval { tv_sec: 0, tv_usec: 0 }];
            // SAFETY: p is a valid path string and times points at two timevals.
            assert_eq!(unsafe { libc::utimes(p.as_ptr(), times.as_ptr()) }, 0);
        }
        let took = with_lock(cache.path(), out.path(), short, Duration::from_millis(1000), || Ok("took over")).unwrap();
        assert_eq!(took, "took over");
        Ok(())
    })
    .unwrap();
}

#[test]
fn check_file_on_a_missing_file_is_a_usage_error() {
    let home = temp_dir();
    let tmp = temp_dir();
    let run = cli(&["check", "--file", &tmp.join("nope.html")], ROOT, home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("Not found"));
}

#[test]
fn svg_attributes_cannot_spell_url_with_entities_or_css_escapes() {
    let html = example_html();
    for svg in [
        r#"<svg><rect fill="url&lpar;https://example.com/p.svg#g&rpar;"/></svg>"#,
        r#"<svg><rect fill="u\72l(https://example.com/p.svg#g)"/></svg>"#,
        r#"<svg><rect clip-path="url (https://example.com/c.svg#c)"/></svg>"#,
        r#"<svg><rect stroke="url(https://example.com/s.svg#s)"/></svg>"#,
        "<svg><rect fill=\"u\\72\r\nl(https://example.com/p.svg#g)\"/></svg>",
        r#"<svg><rect fill="u\72&#13;&#10;l(https://example.com/p.svg#g)"/></svg>"#,
        "<style>body{background:u\\72\r\nl(https://example.com/p.gif)}</style>",
        "<style>@im\\70\r\nort \"x.css\";</style>",
    ] {
        assert!(caught(&html, svg), "{svg}");
    }
    assert_eq!(
        clean(
            &html,
            r#"<svg viewBox="0 0 10 10"><defs><lineargradient id="g"></lineargradient></defs><rect fill="url(#g)" transform="translate(1 2)"/></svg>"#
        ),
        Vec::<String>::new()
    );
    assert_eq!(clean(&html, r#"<p title="R&D and Q&A">x</p>"#), Vec::<String>::new());
}

#[test]
fn an_example_build_still_refuses_a_registered_project_s_folder() {
    let r = registered("tidewater");
    let elsewhere = temp_dir();
    let run = cli(&["build", &example(), "--example", "--out", &format!("{}/{NAME}", r.repo)], elsewhere.path(), r.home.path());
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("inside a repository"), "{}", run.stderr);
}

#[test]
fn escaped_quotes_and_backslashes_cannot_move_where_a_css_url_starts() {
    let html = example_html();
    for css in [r"a{b:url(\5c data:x)}", r"a{b:url(\22 data:x)}", r"a{b:url(\000027data:x)}", r#"a{content:"\""}"#] {
        assert!(caught(&html, &format!("<style>{css}</style>")), "{css}");
    }
    assert_eq!(clean(&html, r#"<style>li::before{content:"\2013"}a{b:url(data:x)}</style>"#), Vec::<String>::new());
}

#[test]
fn only_css_whitespace_may_pad_a_url_not_a_no_break_or_zero_width_space() {
    let html = example_html();
    for snippet in [
        r"<style>a{b:url(\a0 data:x)}</style>",
        "<style>a{b:url(\u{a0}data:x)}</style>",
        "<style>a{b:url(\"\u{feff}data:x\")}</style>",
        "<svg><rect fill=\"url(\u{a0}#g)\"/></svg>",
    ] {
        assert!(caught(&html, snippet), "{snippet}");
    }
    assert_eq!(clean(&html, r#"<style>a{b:url( data:x)}</style><svg><rect fill="url( #g )"/></svg>"#), Vec::<String>::new());
}
