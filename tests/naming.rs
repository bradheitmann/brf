// Ported from test/naming.test.mjs.
use brf::naming::{Kind, META_KEY, Parsed, format_parsed, human_date, is_brief, is_real_date, parse_name, to_repo_name};

#[test]
fn parses_project_meta_same_day_and_variant_names() {
    assert_eq!(
        parse_name("brf_mister-clean_20260923.html"),
        Some(Parsed {
            kind: Kind::Project,
            key: "mister-clean".into(),
            repo: Some("mister-clean".into()),
            date: "20260923".into(),
            v: None,
            variant: None,
            ext: "html".into()
        })
    );
    let meta = parse_name("meta_brf_20260923.html").unwrap();
    assert_eq!(meta.kind, Kind::Meta);
    assert_eq!(meta.key, META_KEY);
    assert_eq!(parse_name("brf_harbor-log_20260923-v2.html").unwrap().v, Some(2));
    let pdf = parse_name("brf_tidewater_20260914_dark.pdf");
    assert_eq!(pdf.as_ref().unwrap().variant.as_deref(), Some("dark"));
    assert!(!is_brief(pdf.as_ref()));
    assert!(is_brief(parse_name("brf_tidewater_20260914.html").as_ref()));
}

#[test]
fn rejects_names_that_break_the_convention() {
    for bad in [
        "brf_Tide_20260923.html",
        "brf_tide_2026-09-23.html",
        "brf_tide_20260231.html",
        "2026-09-23-tide-briefing.html",
        "brf__20260923.html",
        "brf_tide_water_20260923.html",
    ] {
        assert_eq!(parse_name(bad), None, "{bad}");
    }
}

#[test]
fn formats_what_it_parses() {
    for n in ["brf_a-b_20260101.html", "meta_brf_20261231-v3.html", "brf_x_20260102_audio.m4a"] {
        let p = parse_name(n).unwrap();
        assert_eq!(format_parsed(&p, p.v), n);
    }
}

#[test]
fn derives_repository_names() {
    assert_eq!(to_repo_name("Harbor_Log_Service"), "harbor-log-service");
    assert_eq!(to_repo_name("tide.water.git"), "tide-water");
    assert_eq!(to_repo_name("--Odd  Name!!"), "odd-name");
}

#[test]
fn knows_real_dates() {
    assert!(is_real_date("20240229"));
    assert!(!is_real_date("20250229"));
    assert_eq!(human_date("20261102"), "2 November 2026");
}
