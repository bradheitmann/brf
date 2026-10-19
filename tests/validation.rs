use brf::validation::{BriefInput, validate};

#[test]
fn validates_borrowed_content_without_opening_the_name() {
    let html = r#"<meta name="brf-version" content="2.1.0"><meta name="brf-template" content="project 2.0.1"><meta name="brf-brief" content="1"><meta name="brf-repo" content="example"><meta name="brf-date" content="2026-10-19"><main><p>Ready for review.</p></main>"#;
    let report = validate(BriefInput { html, name: "brf_example_20261019.html", banned_extra: &[] });
    assert!(report.is_valid(), "{:?}", report.problems);
    let hostile = html.replace("Ready for review.", "<script>alert(1)</script><img src=outside>");
    let denied = validate(BriefInput { html: &hostile, name: "brf_example_20261019.html", banned_extra: &[] });
    assert!(!denied.is_valid());
    assert!(denied.problems.iter().any(|p| p.contains("script")));
    assert!(denied.problems.iter().any(|p| p.contains("img")));
}

#[cfg(feature = "full")]
#[test]
fn compatibility_path_preserves_all_diagnostics_and_order() {
    let html = "<main><script>bad</script><p>honestly TASK-123</p></main>";
    let extra = vec!["bad".to_owned()];
    let input = BriefInput { html, name: "brf_example_20261019.html", banned_extra: &extra };
    assert_eq!(validate(input).problems, brf::check::check_html(html, "brf_example_20261019.html", &extra));
}

#[cfg(feature = "full")]
#[test]
fn legacy_scanner_types_and_allowlist_are_still_public() {
    let attr = brf::check::Attr { name: "class".into(), raw: "brief".into(), value: "brief".into() };
    let tag = brf::check::Tag { name: "main".into(), attrs: vec![attr], foreign: false };
    assert!(brf::check::allowlist_problems(&[tag], &[]).is_empty());
    assert_eq!(brf::check::DEFAULT_BANNED, brf::validation::DEFAULT_BANNED);
    let (tags, styles) = brf::check::scan_markup("<script>alert(1)</script>");
    assert_eq!(brf::check::allowlist_problems(&tags, &styles), brf::validation::markup_problems("<script>alert(1)</script>"));
}
