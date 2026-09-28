// Ported from test/build.test.mjs.
mod common;
use brf::build::BuildOptions;
use brf::html::read_stamp;
use common::*;
use serde_json::json;

#[test]
fn builds_the_example_fonts_in_comments_out_versions_stamped() {
    let home = temp_dir();
    let env = test_env(home.path());
    let dir = temp_dir();
    let out = dir.join("brf_tidewater_20261102.html");
    let ex = example();
    let r = build_quiet(&BuildOptions { brief: Some(2.0), example: true, ..opts(&ex, &out, &env) }).unwrap();
    let html = read(&r.path);
    assert!(!html.contains("<!--"));
    assert!(!re(r"\{\{[A-Z0-9_]+\}\}").is_match(&html));
    assert!(html.contains("@font-face"));
    let stamp = read_stamp(&html);
    assert_eq!(stamp.brief, Some(2));
    assert_eq!(stamp.repo.as_deref(), Some("tidewater"));
    assert_eq!(stamp.template.as_deref(), Some("project 2.0.0"));
    assert_eq!(stamp.brf_version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
    assert!(re(r"Brief 2 of this project · brf \d+\.\d+\.\d+ · project template 2\.0\.0").is_match(&html));
}

#[test]
fn refuses_unfilled_slots_a_missing_template_tag_a_bad_name_and_a_kind_mismatch() {
    let home = temp_dir();
    let env = test_env(home.path());
    let dir = temp_dir();
    let out = dir.join("brf_tidewater_20261102.html");
    let ex = example();
    let template = root_file("templates/project.html");
    assert!(err_text(build_quiet(&BuildOptions { example: true, ..opts(&template, &out, &env) })).contains("still to do"));
    let no_tag = dir.join("no-tag.filled.html");
    write(&no_tag, &re(r#"<meta name="brf-template"[^>]*>"#).replace(&read(&ex), ""));
    assert!(err_text(build_quiet(&BuildOptions { example: true, ..opts(&no_tag, &out, &env) })).contains("brf-template"));
    let bad = dir.join("tidewater.html");
    assert!(err_text(build_quiet(&BuildOptions { example: true, ..opts(&ex, &bad, &env) })).contains("not a brief name"));
    let meta = dir.join("meta_brf_20261102.html");
    assert!(err_text(build_quiet(&BuildOptions { example: true, ..opts(&ex, &meta, &env) })).contains("cannot become"));
}

#[test]
fn refuses_an_unregistered_project_and_numbers_from_the_output_folder() {
    let home = temp_dir();
    let env = test_env(home.path());
    let out = temp_dir();
    write_registry(out.path(), json!([]));
    let dir = temp_dir();
    let target = dir.join("brf_tidewater_20261102.html");
    let ex = example();
    let o = BuildOptions { output_dir: Some(out.path()), ..opts(&ex, &target, &env) };
    assert!(err_text(build_quiet(&o)).contains("not in the registry"));
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }]));
    assert_eq!(build_quiet(&o).unwrap().brief, 1);
}

#[test]
fn refuses_to_rebuild_a_finished_page() {
    let home = temp_dir();
    let env = test_env(home.path());
    let dir = temp_dir();
    let ex = example();
    let first_out = dir.join("brf_tidewater_20261102.html");
    let first = build_quiet(&BuildOptions { example: true, ..opts(&ex, &first_out, &env) }).unwrap();
    let again = dir.join("brf_tidewater_20261103.html");
    assert!(err_text(build_quiet(&BuildOptions { example: true, ..opts(&first.path, &again, &env) })).contains("already a finished brief"));
}
