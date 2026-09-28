// Ported from test/templates-dir.test.mjs.
mod common;
use brf::build::BuildOptions;
use brf::templates::{builtin_templates_dir, template_path, templates_dir};
use common::*;

#[test]
fn an_owner_templates_folder_replaces_the_built_in_templates_and_fonts_file_by_file() {
    let own = temp_dir();
    let home = temp_dir();
    let mut env = test_env(home.path());
    env.set("BRF_TEMPLATES_DIR", own.path());
    let builtin = builtin_templates_dir();
    assert_eq!(template_path("project.html", &env), format!("{builtin}/project.html"), "falls back while the folder is empty");
    write(&own.join("project.html"), &read(&root_file("templates/project.html")));
    write(&own.join("fonts.css"), "/* owner fonts */");
    assert_eq!(templates_dir(&env).as_deref(), Some(own.path()));
    assert_eq!(template_path("project.html", &env), own.join("project.html"));
    assert_eq!(template_path("meta.html", &env), format!("{builtin}/meta.html"), "a kind the folder lacks stays built in");
    assert_eq!(template_path("fonts.css", &env), own.join("fonts.css"));
}

#[test]
fn a_build_embeds_the_owner_s_fonts() {
    let own = temp_dir();
    write(&own.join("fonts.css"), "/* owner-fonts-marker */");
    let home = temp_dir();
    let mut env = test_env(home.path());
    env.set("BRF_TEMPLATES_DIR", own.path());
    let dir = temp_dir();
    let out = dir.join("brf_tidewater_20261102.html");
    let ex = example();
    let r = build_quiet(&BuildOptions { example: true, ..opts(&ex, &out, &env) }).unwrap();
    assert!(read(&r.path).contains("owner-fonts-marker"));
}
