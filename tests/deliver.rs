// Ported from test/deliver.test.mjs.
mod common;
use brf::build::BuildOptions;
use brf::deliver::deliver;
use brf::folder::{ensure_layout, history, next_number};
use common::*;
use serde_json::{Value, json};

struct Ctx {
    out: TempDir,
    work: TempDir,
    cache: TempDir,
    home: TempDir,
}

fn setup() -> Ctx {
    let out = temp_dir();
    ensure_layout(out.path()).unwrap();
    write_registry(out.path(), json!([{ "repo_name": "tidewater" }, { "repo_name": "harbor-log" }]));
    Ctx { out, work: temp_dir(), cache: temp_dir(), home: temp_dir() }
}

fn build_at(ctx: &Ctx, date: &str) -> String {
    let env = test_env(ctx.home.path());
    let ex = example();
    let target = ctx.work.join(&format!("brf_tidewater_{date}.html"));
    build_quiet(&BuildOptions { output_dir: Some(ctx.out.path()), ..opts(&ex, &target, &env) }).unwrap().path
}

fn build_and_deliver(ctx: &Ctx, date: &str) -> Value {
    let path = build_at(ctx, date);
    deliver(&path, ctx.out.path(), ctx.cache.path()).unwrap()
}

#[test]
fn files_the_previous_brief_in_the_archive_and_numbers_each_brief() {
    let ctx = setup();
    let first = build_and_deliver(&ctx, "20261012");
    assert_eq!(first["brief"], 1);
    assert_eq!(first["archived"], json!([]));
    let second = build_and_deliver(&ctx, "20261102");
    assert_eq!(second["brief"], 2);
    assert_eq!(second["archived"], json!([{ "from": "brf_tidewater_20261012.html", "to": "archive/brf_tidewater_20261012.html" }]));
    let html: Vec<String> = brf::folder::sorted_names(ctx.out.path()).into_iter().filter(|n| n.ends_with(".html")).collect();
    assert_eq!(html, vec!["brf_tidewater_20261102.html".to_string()]);
    assert_eq!(next_number(ctx.out.path(), "tidewater"), 3);
}

#[test]
fn a_same_day_rebrief_archives_the_earlier_one_as_v1_then_v2() {
    let ctx = setup();
    build_and_deliver(&ctx, "20261102");
    let b = build_and_deliver(&ctx, "20261102");
    assert_eq!(b["archived"][0]["to"], "archive/brf_tidewater_20261102-v1.html");
    let c = build_and_deliver(&ctx, "20261102");
    assert_eq!(c["archived"][0]["to"], "archive/brf_tidewater_20261102-v2.html");
    assert_eq!(history(ctx.out.path(), "tidewater").iter().map(|h| h.number).collect::<Vec<_>>(), vec![1, 2, 3]);
}

#[test]
fn touches_no_other_project_s_files() {
    let ctx = setup();
    let other = ctx.out.join("brf_harbor-log_20261001.html");
    write(&other, "<p>other</p>");
    build_and_deliver(&ctx, "20261102");
    build_and_deliver(&ctx, "20261103");
    assert_eq!(read(&other), "<p>other</p>");
}

#[test]
fn refuses_a_stale_brief_number_an_unbuilt_file_and_an_older_date() {
    let ctx = setup();
    let early = build_at(&ctx, "20261101");
    let late = build_at(&ctx, "20261102");
    deliver(&late, ctx.out.path(), ctx.cache.path()).unwrap();
    assert!(err_text(deliver(&early, ctx.out.path(), ctx.cache.path())).contains("next brief"));
    let raw = ctx.work.join("brf_tidewater_20261104.html");
    write(&raw, "<html></html>");
    assert!(err_text(deliver(&raw, ctx.out.path(), ctx.cache.path())).contains("no complete brf stamp"));
    let older = build_at(&ctx, "20261030");
    assert!(err_text(deliver(&older, ctx.out.path(), ctx.cache.path())).contains("later date"));
    assert!(std::path::Path::new(&ctx.out.join("brf_tidewater_20261102.html")).exists());
}
