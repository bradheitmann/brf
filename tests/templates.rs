// Ported from test/templates.test.mjs.
mod common;
use brf::templates::{brf_version, current_templates};
use common::*;

#[test]
fn every_template_declares_a_version_and_the_changelog_records_it() {
    let changelog = read(&root_file("CHANGELOG.md"));
    assert!(re(&format!(r"## brf {}\b", regex::escape(brf_version()))).is_match(&changelog), "CHANGELOG.md has no ## brf {}", brf_version());
    let home = temp_dir();
    for t in current_templates(&test_env(home.path())).unwrap() {
        let v = t.version.clone().unwrap_or_default();
        assert!(re(r"^\d+\.\d+\.\d+$").is_match(&v), "{}", t.kind);
        assert!(changelog.contains(&format!("{} {v}", t.kind)), "{} {v} missing from CHANGELOG.md", t.kind);
    }
}

#[test]
fn templates_carry_the_fonts_marker_and_the_project_template_marks_its_appendices() {
    let project = read(&root_file("templates/project.html"));
    let meta = read(&root_file("templates/meta.html"));
    for t in [&project, &meta] {
        assert!(t.contains("/* {{FONTS_CSS}} */"));
    }
    assert!(project.contains(r#"<section data-brf="appendix">"#));
}

fn repo_files(dir: &str, out: &mut Vec<String>) {
    for name in brf::folder::sorted_names(dir) {
        if [".git", "node_modules", "proof", "target"].contains(&name.as_str()) {
            continue;
        }
        let p = format!("{dir}/{name}");
        if std::path::Path::new(&p).is_dir() {
            repo_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

#[test]
fn nothing_in_the_repository_points_at_a_personal_machine_or_address() {
    let mut files = Vec::new();
    repo_files(ROOT, &mut files);
    assert!(files.len() >= 30, "found only {} files", files.len());
    let personal = re(r"/Users/[A-Za-z]|/home/[a-z]+/|C:\\Users\\");
    let email = re(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9-]+\.[A-Za-z0-9.-]*[a-z]{2,}");
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        assert!(!personal.is_match(&text), "{f} has a personal path");
        if f.ends_with("fonts.css") {
            continue;
        }
        assert!(!email.is_match(&text), "{f} has an email address");
    }
}
