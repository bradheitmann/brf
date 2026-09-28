// Ported from test/check.test.mjs, plus the current-brief count fix of brf 2.0.0.
mod common;
use brf::build::BuildOptions;
use brf::check::{check_folder, check_html};
use brf::deliver::deliver;
use brf::folder::ensure_layout;
use common::*;
use serde_json::json;

const NAME: &str = "brf_tidewater_20261102.html";

fn built() -> String {
    let home = temp_dir();
    built_example(&test_env(home.path()), Some(2.0))
}

fn check(html: &str) -> Vec<String> {
    check_html(html, NAME, &[])
}

#[test]
fn the_shipped_example_passes_every_check() {
    assert_eq!(check(&built()), Vec::<String>::new());
}

#[test]
fn catches_scripts_network_fetches_em_dashes_and_filler_words() {
    let html = built().replacen("</body>", "<script>alert(1)</script><img src=\"https://example.com/x.png\"></body>", 1).replacen(
        "Tidewater is live",
        "Tidewater is honestly live \u{2014} mostly",
        1,
    );
    let problems = check(&html).join("\n");
    assert!(problems.contains("script element"), "{problems}");
    assert!(problems.contains("<img> element, which brf does not allow"), "{problems}");
    assert!(problems.contains("em-dash"), "{problems}");
    assert!(problems.contains("banned words: honestly"), "{problems}");
}

#[test]
fn flags_identifiers_above_the_appendices_but_not_inside_them() {
    let html = built();
    assert!(check(&html).iter().all(|p| !p.contains("above the appendices")));
    let leaked = html.replacen("Tidewater is live", "Tidewater (TW-12, commit 4f2a9c1, #31, src/time.ts, v2.1.0) is live", 1);
    let problems = check(&leaked).join("\n");
    for what in ["ticket or task id", "commit hash", "pull request", "file path", "version string"] {
        assert!(problems.contains(what), "{what} missing from {problems}");
    }
}

#[test]
fn checks_the_folder_names_registry_one_current_brief_each() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }, { "repo_name": "harbor-log" }]));
    let work = temp_dir();
    let target = work.join(NAME);
    let ex = example();
    let r = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &target, &env) }).unwrap();
    let cache = temp_dir();
    deliver(&r.path, out.path(), cache.path()).unwrap();
    let result = check_folder(out.path(), &[], 14, "20261102").unwrap();
    assert_eq!(result.problems, Vec::<String>::new());
    assert!(result.notes.join("\n").contains("harbor-log: registered, no current brief"));
    write(&out.join("notes.txt"), "x");
    write(&out.join("brf_stranger_20261102.html"), "x");
    let result = check_folder(out.path(), &[], 14, "20261102").unwrap();
    let problems = result.problems.join("\n");
    assert!(problems.contains("notes.txt: not a brf file name"), "{problems}");
    assert!(problems.contains("\"stranger\" is not in the registry"), "{problems}");
}

#[test]
fn a_radio_and_label_theme_switch_passes_other_inputs_forms_and_buttons_do_not() {
    let html = built();
    let toggle = concat!(
        r#"<input type="radio" name="t" id="t-light" class="vh" checked aria-label="Light theme">"#,
        r#"<input type="radio" name="t" id="t-dark" class="vh" aria-label="Dark theme">"#,
        r#"<label for="t-dark" class="theme" title="Switch" aria-label="Switch to dark">Light</label>"#
    );
    assert_eq!(check(&html.replacen("</main>", &format!("{toggle}</main>"), 1)), Vec::<String>::new());
    assert_eq!(check(&html.replacen("</main>", r#"<input type="RADIO" name="t" id="x"></main>"#, 1)), Vec::<String>::new());
    let refused = [
        ("text input", r#"<input type="text" name="q">"#),
        ("image input", r#"<input type="image" src="https://example.com/a.png">"#),
        ("file input", r#"<input type="file">"#),
        ("submit input", r#"<input type="submit" formaction="https://example.com">"#),
        ("input without a type", r#"<input name="q">"#),
        ("two types", r#"<input type="radio" type="image" src="https://example.com/a.png">"#),
        ("radio with src", r#"<input type="radio" src="https://example.com/a.png">"#),
        ("radio with a value", r#"<input type="radio" name="t" value="x">"#),
        ("radio with style", r#"<input type="radio" style="width:1px">"#),
        ("radio with form", r#"<input type="radio" form="f">"#),
        ("radio with an event", r#"<input type="radio" onchange="fetch(1)">"#),
        ("label with an event", r#"<label for="t" onclick="fetch(1)">x</label>"#),
        ("label with a style", r#"<label for="t" style="width:1px">x</label>"#),
        ("label with form", r#"<label for="t" form="f">x</label>"#),
        ("form", r#"<form action="https://example.com"><input type="radio"></form>"#),
        ("button", r#"<button type="button">x</button>"#),
        ("select", "<select><option>x</option></select>"),
        ("textarea", "<textarea>x</textarea>"),
    ];
    for (what, snippet) in refused {
        assert!(!check(&html.replacen("</main>", &format!("{snippet}</main>"), 1)).is_empty(), "{what} was not caught");
    }
}

#[test]
fn the_build_writes_the_brief_number_into_the_page() {
    let html = built();
    assert!(!html.contains("{{BRIEF_NUMBER}}"));
    assert!(html.contains("Brief 2<"));
}

#[test]
fn the_shipped_meta_example_builds_and_passes_every_check() {
    let home = temp_dir();
    let env = test_env(home.path());
    let dir = temp_dir();
    let out = dir.join("meta_brf_20261102.html");
    let ex = root_file("examples/meta.filled.html");
    let r = build_quiet(&BuildOptions { example: true, ..opts(&ex, &out, &env) }).unwrap();
    assert_eq!(check_html(&read(&r.path), "meta_brf_20261102.html", &[]), Vec::<String>::new());
}

#[test]
fn the_current_brief_count_leaves_out_the_meta_brief() {
    // brf 1.x counted the meta brief as a project brief: "3 current brief(s)" for two projects.
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }, { "repo_name": "harbor-log" }]));
    let work = temp_dir();
    let cache = temp_dir();
    let ex = example();
    let project = work.join(NAME);
    let r = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &project, &env) }).unwrap();
    deliver(&r.path, out.path(), cache.path()).unwrap();
    let meta_src = root_file("examples/meta.filled.html");
    let meta = work.join("meta_brf_20261102.html");
    let m = build_quiet(&BuildOptions { output_dir: Some(out.path()), ..opts(&meta_src, &meta, &env) }).unwrap();
    deliver(&m.path, out.path(), cache.path()).unwrap();

    let result = check_folder(out.path(), &[], 14, "20261102").unwrap();
    assert_eq!(result.problems, Vec::<String>::new());
    assert_eq!(result.current_count, 1, "one project brief; the meta brief is not a project brief");
    assert_eq!(result.registered_count, 2);

    // The command line says the same.
    let r = cli_env(&["check"], home.path(), home.path(), &[("BRF_OUTPUT_DIR", out.path())]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("1 current project brief(s), 2 registered project(s)"), "{}", r.stdout);
    let r = cli_env(&["check", "--json"], home.path(), home.path(), &[("BRF_OUTPUT_DIR", out.path())]);
    assert_eq!(r.json()["current_count"], 1);
}
