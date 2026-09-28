// Parity with brf 1.2.0, the Node command line this binary replaces.
//
// Every test here runs the old CLI and the new binary on the same fixtures, at the same paths,
// and compares what they print, what they exit with and what they write: check results, built
// pages, snapshot JSON, registry and config writes, delivery and archiving. The only differences
// allowed are the brf version number (1.2.0 against 2.0.0), timestamps, the folder brf runs from,
// and the one fix in 2.0.0: the folder check counts project briefs only.
//
// The old CLI is found at $BRF_PARITY_NODE_CLI (a path to bin/brf.mjs), else bin/brf.mjs in this
// checkout, else it is extracted from the last commit in this repository's history that has it.
// Without Node or the old CLI the tests say so and pass; set BRF_PARITY_REQUIRED=1 to fail instead.
//
// `cargo test --test parity -- --ignored` with BRF_PARITY_REAL_DIR=<a brief folder> also compares
// both on a copy of a real brief folder. The copy is made in a temporary folder and removed after.
mod common;
use common::*;
use regex::Regex;
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

struct Old {
    /// The folder that holds bin/brf.mjs, lib/ and templates/.
    root: String,
    _keep: Option<TempDir>,
}

fn node_ok() -> bool {
    Command::new("node").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

fn extract_from_history() -> Option<(String, TempDir)> {
    let last = git_out(ROOT, &["log", "-1", "--format=%H", "--", "bin/brf.mjs"]).trim().to_string();
    if last.is_empty() {
        return None;
    }
    let has = |c: &str| Command::new("git").args(["cat-file", "-e", &format!("{c}:bin/brf.mjs")]).current_dir(ROOT).status().is_ok_and(|s| s.success());
    let commit = if has(&last) { last } else { format!("{last}^") };
    if !has(&commit) {
        return None;
    }
    let dir = temp_dir();
    let archive = Command::new("git")
        .args(["archive", "--format=tar", &commit, "bin", "lib", "package.json", "templates"])
        .current_dir(ROOT)
        .stdout(Stdio::piped())
        .spawn()
        .ok()?;
    let status = Command::new("tar").args(["-x", "-C", dir.path()]).stdin(archive.stdout?).status().ok()?;
    status.success().then(|| (dir.path().to_string(), dir))
}

fn old() -> Option<&'static Old> {
    static OLD: OnceLock<Option<Old>> = OnceLock::new();
    OLD.get_or_init(|| {
        if !node_ok() {
            return None;
        }
        if let Ok(cli) = std::env::var("BRF_PARITY_NODE_CLI") {
            let root = Path::new(&cli).parent()?.parent()?.to_str()?.to_string();
            return Some(Old { root, _keep: None });
        }
        if Path::new(&root_file("bin/brf.mjs")).exists() {
            return Some(Old { root: ROOT.to_string(), _keep: None });
        }
        extract_from_history().map(|(root, keep)| Old { root, _keep: Some(keep) })
    })
    .as_ref()
}

/// The old CLI, or a skip (a failure when BRF_PARITY_REQUIRED=1).
macro_rules! old_or_skip {
    () => {
        match old() {
            Some(o) => o,
            None => {
                if std::env::var("BRF_PARITY_REQUIRED").is_ok_and(|v| v == "1") {
                    panic!("parity needs node and the brf 1.2.0 CLI (set BRF_PARITY_NODE_CLI)");
                }
                eprintln!("parity skipped: node or the brf 1.2.0 CLI is not available");
                return;
            }
        }
    };
}

#[derive(Debug, PartialEq)]
struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

struct Pair<'a> {
    old: &'a Old,
    home: &'a str,
    extra: Vec<(String, String)>,
}

impl Pair<'_> {
    fn extra(&self) -> Vec<(&str, &str)> {
        self.extra.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }

    fn node(&self, args: &[&str], cwd: &str) -> Out {
        let bin = format!("{}/bin/brf.mjs", self.old.root);
        let r = run_with("node", &[&bin], args, cwd, self.home, &self.extra());
        Out { code: r.code, stdout: normalize(&r.stdout, self.old), stderr: normalize(&r.stderr, self.old) }
    }

    fn rust(&self, args: &[&str], cwd: &str) -> Out {
        let r = run_with(BIN, &[], args, cwd, self.home, &self.extra());
        Out { code: r.code, stdout: normalize(&r.stdout, self.old), stderr: normalize(&r.stderr, self.old) }
    }
}

/// Take out what may differ: the brf version, timestamps and the folder brf runs from.
fn normalize(s: &str, old: &Old) -> String {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        vec![
            (Regex::new(r#"<meta name="brf-version" content="[^"]*">"#).unwrap(), r#"<meta name="brf-version" content="<V>">"#),
            (Regex::new(r#""brf_version": "[^"]*""#).unwrap(), r#""brf_version": "<V>""#),
            (Regex::new(r"brf \d+\.\d+\.\d+").unwrap(), "brf <V>"),
            (Regex::new(r#""taken_at": "[^"]*""#).unwrap(), r#""taken_at": "<T>""#),
            (Regex::new(r#""generated_at": "[^"]*""#).unwrap(), r#""generated_at": "<T>""#),
        ]
    });
    let mut out = s.replace(&format!("{}/", old.root), "<BRF>/").replace(&format!("{ROOT}/"), "<BRF>/");
    for (re, to) in rules {
        out = re.replace_all(&out, *to).into_owned();
    }
    out
}

fn assert_same(what: &str, a: &Out, b: &Out) {
    assert_eq!(a.code, b.code, "{what}: exit codes differ\nnode: {a:?}\nrust: {b:?}");
    assert_eq!(a.stdout, b.stdout, "{what}: stdout differs");
    assert_eq!(a.stderr, b.stderr, "{what}: stderr differs");
}

/// Run both and require identical results.
fn both(p: &Pair<'_>, what: &str, args: &[&str], cwd: &str) -> Out {
    let a = p.node(args, cwd);
    let b = p.rust(args, cwd);
    assert_same(&format!("{what} ({})", args.join(" ")), &a, &b);
    b
}

/// Every file under a folder with its content, for comparing whole trees.
fn tree(dir: &str, old: &Old) -> Vec<(String, String)> {
    fn walk(base: &str, dir: &str, out: &mut Vec<(String, String)>, old: &Old) {
        for name in brf::folder::sorted_names(dir) {
            let p = format!("{dir}/{name}");
            let rel = p[base.len()..].to_string();
            if Path::new(&p).is_dir() {
                out.push((format!("{rel}/"), String::new()));
                walk(base, &p, out, old);
            } else if !name.starts_with('.') {
                let text = String::from_utf8_lossy(&std::fs::read(&p).unwrap()).into_owned();
                out.push((rel, normalize(&text, old)));
            }
        }
    }
    let mut out = Vec::new();
    if Path::new(dir).exists() {
        walk(dir, dir, &mut out, old);
    }
    out
}

fn reset(dir: &str) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
}

// ---------------------------------------------------------------------------------------------

/// Snippets injected before </main> of the built example: every case from the check tests, and
/// more edge cases for the tokenizer, the CSS reader and the identifier rules.
fn corpus() -> Vec<String> {
    let at = "@";
    let mut v: Vec<String> = [
        "",
        "<script>alert(1)</script>",
        r#"<img src="https://example.com/x.png">"#,
        "<p>Tidewater is honestly live \u{2014} mostly</p>",
        "<p>Tidewater (TW-12, commit 4f2a9c1, #31, src/time.ts, v2.1.0) is live</p>",
        r#"<input type="radio" name="t" id="t-light" class="vh" checked aria-label="Light theme"><label for="t-dark" class="theme" title="Switch" aria-label="Switch to dark">Light</label>"#,
        r#"<input type="RADIO" name="t" id="x">"#,
        r#"<input type="text" name="q">"#,
        r#"<input type="image" src="https://example.com/a.png">"#,
        r#"<input type="submit" formaction="https://example.com">"#,
        r#"<input name="q">"#,
        r#"<input type="radio" type="image" src="https://example.com/a.png">"#,
        r#"<input type="radio" name="t" value="x">"#,
        r#"<input type="radio" style="width:1px">"#,
        r#"<input type="radio" onchange="fetch(1)">"#,
        r#"<label for="t" onclick="fetch(1)">x</label>"#,
        r#"<label for="t" form="f">x</label>"#,
        r#"<form action="https://example.com"><input type="radio"></form>"#,
        r#"<button type="button">x</button>"#,
        "<select><option>x</option></select>",
        "<textarea>x</textarea>",
        r#"<svg><image href="https://example.com/a.png"/></svg>"#,
        r#"<img srcset="https://example.com/a.png 2x">"#,
        r#"<meta http-equiv="refresh" content="0;url=https://example.com">"#,
        r#"<a href="javascript:alert(1)">x</a>"#,
        "<iframe></iframe>",
        r#"<link rel="stylesheet" href="//example.com/a.css">"#,
        r#"<a href="https://example.com">a link</a>"#,
        r#"<img/src="x.png"/onerror="alert(1)">"#,
        r#"<svg/onload="alert(1)"></svg>"#,
        r#"<img src="x.png"onerror="alert(1)">"#,
        r#"<img alt="a>b" src="x.png" onerror="alert(1)">"#,
        "<img src=x alt=it's onerror=alert(1)>",
        r#"<img src=x title=a"b onerror=alert(1)//">"#,
        r#"<a href="&#106;avascript:alert(1)">x</a>"#,
        r#"<img src="https:&#47;&#47;example.com/a.png">"#,
        "<a href=\"java\tscript:alert(1)\">x</a>",
        r#"<p title="turn online=yes">text</p>"#,
        r#"<p title="set src=https://example.com">text</p>"#,
        "<!--> <img src=x onerror=alert(1)> -->",
        "<!---> <img src=x onerror=alert(1)> -->",
        "<!-- a --!> <img src=x onerror=alert(1)> -->",
        r#"<img alt="<!--" src=x onerror=alert(1)><p title="-->">"#,
        r#"<style>/* <a title=" */</style><img src=x onerror=alert(1)><style>/* " */</style>"#,
        r#"<!x <a title=" ><img src=x onerror=alert(1)><b title=x">"#,
        "<p title=\u{a0}\"a onerror=alert(1) b=x\">x</p>",
        r#"<p class="https&colon;//example.com">x</p><img src="https&colon;//example.com/a.png">"#,
        r#"<a href="https&colon;//example.com">x</a>"#,
        r#"<link rel=preload as=image imagesrcset="https://example.com/a.png">"#,
        r"<style>body{background:url(\68ttps://example.com/a.png)}</style>",
        "<style>body{background:url(https:example.com/a.png)}</style>",
        "<svg><style>rect{fill:url(&#104;ttps://example.com/a)}</style></svg>",
        r#"<p style="background:url(https://example.com/a.png)">x</p>"#,
        "<p>The fonts came in with @import once; now they are embedded.</p>",
        "<p>If a &lt; b and x&lt;y, nothing happens &#9999999; at all.</p>",
        r#"<a href="https://example.com/?a=1&amp;b=2">a link</a>"#,
        r#"<a href="&#104ttps://example.com/&#9999999;">a link written with odd entities</a>"#,
        r#"<style>.a{content:"/*"}body{background:url(https://example.com/a.png)}.b{content:"*/"}</style>"#,
        r"<style>.a{b:\/*}body{background:url(https://example.com/a.png)}.c{d:*/}</style>",
        r#"<svg><rect fill="url&lpar;https://example.com/p.svg#g&rpar;"/></svg>"#,
        r#"<svg><rect fill="u\72l(https://example.com/p.svg#g)"/></svg>"#,
        r#"<svg><rect clip-path="url (https://example.com/c.svg#c)"/></svg>"#,
        "<svg><rect fill=\"u\\72\r\nl(https://example.com/p.svg#g)\"/></svg>",
        r#"<svg><rect fill="u\72&#13;&#10;l(https://example.com/p.svg#g)"/></svg>"#,
        "<style>body{background:u\\72\r\nl(https://example.com/p.gif)}</style>",
        "<style>@im\\70\r\nort \"x.css\";</style>",
        r#"<svg viewBox="0 0 10 10"><defs><lineargradient id="g"></lineargradient></defs><rect fill="url(#g)" transform="translate(1 2)"/></svg>"#,
        r#"<p title="R&D and Q&A">x</p>"#,
        r"<style>a{b:url(\5c data:x)}</style>",
        r"<style>a{b:url(\22 data:x)}</style>",
        r"<style>a{b:url(\000027data:x)}</style>",
        r#"<style>a{content:"\""}</style>"#,
        r#"<style>li::before{content:"\2013"}a{b:url(data:x)}</style>"#,
        r"<style>a{b:url(\a0 data:x)}</style>",
        "<style>a{b:url(\u{a0}data:x)}</style>",
        "<style>a{b:url(\"\u{feff}data:x\")}</style>",
        "<svg><rect fill=\"url(\u{a0}#g)\"/></svg>",
        r#"<style>a{b:url( data:x)}</style><svg><rect fill="url( #g )"/></svg>"#,
        // More identifiers, words and odd markup.
        "<p>See ABC-DEF and XY-1234 and A-12 and AB-12a and ab-12 and API-v2.</p>",
        "<p>Hashes: deadbeef, 1234567, abcdefa, 0a1b2c3d4e5f, DEADBEEF1, 4f2a9c1x, 4f2a9c1-4f2a9c1.</p>",
        "<p>Numbers: #1, #12345, #123456, a#12, &#12, (#7), ##8.</p>",
        "<p>Paths: docs/readme.md, ./a/b.rs, a/b.c.json, x/y.yaml, x/y.yml, x/y.txt, /abs/p.ts, p/q.TSX</p>",
        "<p>Versions 1.2.3, v10.0.1, 1.2.3.4, 1.2, v1.2.3-beta, x1.2.3.</p>",
        "<p>We leverage it. Seamless! GENUINELY quietly. Load-bearing earns its place, not just delve. game-changers world-class</p>",
        "<p>Honestly\u{a0}honest; dishonest; honesty; leveraging.</p>",
        "<p>Caf\u{e9}-12 na\u{ef}ve \u{2014}\u{2014} two dashes, &mdash; an entity, &MDASH; upper.</p>",
        "<p>Entities: &amp; &AMP; &nbsp; &hellip &hellip; &#x2014; &#X2014; &#0; &#xD800; &#1114112; &bogus;</p>",
        "<details><summary>More</summary><p>TW-99 inside details</p></details>",
        "<table><tr><td>v9.9.9</td></tr></table>",
        "<SCRIPT>x</SCRIPT>",
        "<ScRiPt src=//e>",
        "<style>x</STYLE ><img src=x>",
        "<style>x</stylex><img src=x></style>",
        "<title>a</title x><img src=x>",
        "<plaintext><img src=x>",
        "<xmp><img src=x></xmp>",
        "<noscript><img src=x></noscript>",
        "<svg><title><img src=x></title></svg>",
        "<svg><foreignObject><p>x</p></foreignObject></svg>",
        "<math><mi>x</mi></math>",
        "<svg/><img src=x>",
        "<svg><svg></svg><script>1</script></svg>",
        "</ p><img src=x>",
        "</3 <img src=x>",
        "<? x <img src=x> ?>",
        "<!DOCTYPE x><img src=x>",
        "<a href=\"mailto:someone\">m</a>",
        "<a href=\"MAILTO:x\">m</a>",
        "<a href=\"HTTPS://EXAMPLE.COM\">m</a>",
        "<a href=\"#top\">m</a>",
        "<a href=\"#\">m</a>",
        "<a href=\"\">m</a>",
        "<a href>m</a>",
        "<a href=\"https://\">m</a>",
        "<a href=\"https:// x\">m</a>",
        "<a href=\"ftp://x\">m</a>",
        "<a href=\"data:text/html,x\">m</a>",
        "<a href=\"https://x\" rel=\"noopener\" target=\"_blank\">m</a>",
        "<a hreflang=en href=\"https://x\">m</a>",
        "<p style=\"width:40%;color:#fff\">x</p>",
        "<p style=\"width:40%;color:rgb(1,2,3)\">x</p>",
        "<p style=\"width&#58;40%\">x</p>",
        "<p STYLE=\"x:y\" CLASS=a ID=b>x</p>",
        "<p data-x=1 aria-hidden=true data-=2>x</p>",
        "<p data-X=1>x</p>",
        "<p x=y>x</p>",
        "<p =x>x</p>",
        "<p \"a\"=b>x</p>",
        "<time datetime=\"2026-01-01\">x</time><ol start=2 reversed><li>x</li></ol>",
        "<td colspan=2>x</td><col span=2><details open><summary>s</summary></details>",
        "<svg><text x=1 font-family=\"a\">t</text><tspan dy=1>u</tspan></svg>",
        "<svg><a href=\"https://x\">l</a></svg>",
        "<svg><use href=\"#x\"/></svg>",
        "<svg><rect filter=\"url(#f)\"/></svg>",
        "<svg><rect fill=\"URL(#g)\"/></svg>",
        "<svg><rect fill=\"url(#g) red\"/></svg>",
        "<svg><rect fill=\"url(#)\"/></svg>",
        "<svg><rect style=\"fill:url(#g)\"/></svg>",
        "<svg><rect fill=\"u&#x72;l(#g)\"/></svg>",
        "<style>a{background:URL( 'data:image/png;base64,AAAA' )}</style>",
        "<style>a{background:url(DATA:x)}</style>",
        "<style>a{background:url(dat)}</style>",
        "<style>a{background:url(\"data:x\")}b{c:url('https://x')}</style>",
        "<style>a{background:image-set(\"a.png\" 1x)}</style>",
        "<style>@IMPORT url(data:x);</style>",
        "<style>a{b:\\}</style>",
        "<style>a{b:\\\\}</style>",
        "<style>a{b:\\0}</style>",
        "<style>a{b:\\110000}</style>",
        "<style>a{b:\\d800 x}</style>",
        "<style>a{b:\\1F600}</style>",
        "<style>a{b:\\\u{1F600}}</style>",
        "<p>\u{1F600} emoji and \u{85} next line and \u{2028} separator</p>",
        "<p>\u{130}stanbul</p><style>a{}</style>",
        "<meta name=x content=\"URL = x\">",
        "<meta charset=utf-8><meta name=\"a\" content=\"b\">",
        "<html data-style=x><body onload=x>",
        "<p onx=1>x</p>",
        "<p ON=1>x</p>",
        "<img",
        "<p title=\"unterminated",
        "<p title='single'>x</p>",
        "<a href=https://x/unquoted>u</a>",
        "<a href=`https://x`>u</a>",
        "<p>&lpar;</p><p title=\"&lpar;\">x</p>",
        "<label for=x title=\"&lpar;\">x</label>",
        "<input type=radio aria-label=\"&amp;\">",
        "<input type=radio aria-label=\"&foo;\">",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.push(format!("<p>Write to billing{at}importfreight.example for invoices.</p>"));
    v
}

fn built_example_node(p: &Pair<'_>, dir: &str) -> String {
    let out = format!("{dir}/brf_tidewater_20261102.html");
    let r = p.node(&["build", &example(), "--out", &out, "--example", "--brief", "2"], dir);
    assert_eq!(r.code, 0, "{r:?}");
    read(&out)
}

#[test]
fn parity_check_on_a_corpus_of_pages() {
    let old = old_or_skip!();
    let home = temp_dir();
    let work = temp_dir();
    let p = Pair { old, home: home.path(), extra: vec![] };
    let html = built_example_node(&p, work.path());
    let mut compared = 0;
    for (i, snippet) in corpus().iter().enumerate() {
        let page = html.replacen("</main>", &format!("{snippet}</main>"), 1);
        let file = work.join(&format!("case{i}/brf_tidewater_20261102.html"));
        std::fs::create_dir_all(Path::new(&file).parent().unwrap()).unwrap();
        write(&file, &page);
        both(&p, &format!("case {i}: {snippet:?}"), &["check", "--file", &file], work.path());
        if i % 10 == 0 {
            both(&p, &format!("case {i} json"), &["check", "--file", &file, "--json"], work.path());
        }
        // The same page under other names: a meta name, another date, a bad name.
        if i % 25 == 0 {
            for name in ["meta_brf_20261102.html", "brf_tidewater_20261103.html", "notes.html"] {
                let other = work.join(&format!("case{i}/{name}"));
                write(&other, &page);
                both(&p, &format!("case {i} as {name}"), &["check", "--file", &other], work.path());
            }
        }
        compared += 1;
    }
    // Owner's extra banned words from the config file.
    let cfg = home.join("cfg/brf/config.json");
    std::fs::create_dir_all(Path::new(&cfg).parent().unwrap()).unwrap();
    write(&cfg, r#"{ "extra_banned_words": ["Tidewater", "time zone"] }"#);
    let file = work.join("case0/brf_tidewater_20261102.html");
    both(&p, "extra banned words", &["check", "--file", &file], work.path());
    assert!(compared > 150, "only {compared} cases");
}

#[test]
fn parity_build_of_the_examples_and_the_templates() {
    let old = old_or_skip!();
    let home = temp_dir();
    let work = temp_dir();
    let p = Pair { old, home: home.path(), extra: vec![] };
    let cases: Vec<(String, &str, Vec<&str>)> = vec![
        (example(), "brf_tidewater_20261102.html", vec!["--example"]),
        (example(), "brf_tidewater_20261102.html", vec!["--example", "--brief", "7"]),
        (root_file("examples/meta.filled.html"), "meta_brf_20261102.html", vec!["--example"]),
        (root_file("examples/meta.filled.html"), "meta_brf_20261102.html", vec!["--example", "--brief=3"]),
        (root_file("templates/project.html"), "brf_tidewater_20261102.html", vec!["--example"]),
        (root_file("templates/meta.html"), "meta_brf_20261102.html", vec!["--example"]),
        (example(), "meta_brf_20261102.html", vec!["--example"]),
        (example(), "tidewater.html", vec!["--example"]),
        (example(), "brf_tidewater_20261102-v1.html", vec!["--example"]),
        (example(), "brf_tidewater_20261102.html", vec!["--example", "--brief", "0"]),
        (example(), "brf_tidewater_20261102.html", vec!["--example", "--brief", "2.5"]),
        (example(), "brf_tidewater_20261102.html", vec!["--example", "--brief", "x"]),
        (example(), "brf_tidewater_20261102.html", vec!["--example", "--brief", ""]),
        (example(), "brf_tidewater_20261102.html", vec![]),
        (work.join("missing.html"), "brf_tidewater_20261102.html", vec!["--example"]),
    ];
    for (i, (input, name, flags)) in cases.iter().enumerate() {
        let a_dir = work.join(&format!("a{i}"));
        let b_dir = work.join(&format!("b{i}"));
        let a_out = format!("{a_dir}/{name}");
        let b_out = format!("{b_dir}/{name}");
        let mut a_args = vec!["build", input.as_str(), "--out", &a_out];
        a_args.extend(flags.iter().copied());
        let mut b_args = vec!["build", input.as_str(), "--out", &b_out];
        b_args.extend(flags.iter().copied());
        let a = p.node(&a_args, work.path());
        let b = p.rust(&b_args, work.path());
        let fix = |o: &Out| Out { code: o.code, stdout: o.stdout.replace(&format!("/b{i}/"), "/X/").replace(&format!("/a{i}/"), "/X/"), stderr: o.stderr.clone() };
        assert_same(&format!("build case {i} {flags:?}"), &fix(&a), &fix(&b));
        assert_eq!(tree(&a_dir, old), tree(&b_dir, old), "build case {i}: written files differ");
        if i < 4 {
            assert!(!tree(&a_dir, old).is_empty(), "build case {i} wrote nothing");
        }
    }
    // A filled file that has been built already, a template from another major, and a missing fonts marker.
    let built = format!("{}/a0/brf_tidewater_20261102.html", work.path());
    let older = work.join("older.filled.html");
    write(&older, &read(&example()).replace("content=\"project 2.0.0\"", "content=\"project 1.4.0\""));
    let newer_minor = work.join("minor.filled.html");
    write(&newer_minor, &read(&example()).replace("content=\"project 2.0.0\"", "content=\"project 2.0.1\""));
    let no_fonts = work.join("nofonts.filled.html");
    write(&no_fonts, &read(&example()).replace("/* {{FONTS_CSS}} */", ""));
    for (i, input) in [built, older, newer_minor, no_fonts].iter().enumerate() {
        let out = work.join(&format!("c{i}/brf_tidewater_20261104.html"));
        both(&p, &format!("refused build {i}"), &["build", input, "--out", &out, "--example"], work.path());
    }
}

#[test]
fn parity_of_the_whole_flow_registry_snapshot_build_deliver_check() {
    let old = old_or_skip!();
    let base = temp_dir();
    let repo_parent = temp_dir();
    let repo = git_repo(&repo_parent.join("tidewater"), Some("https://github.com/acme/Tidewater.git"));
    let other_parent = temp_dir();
    let other = git_repo(&other_parent.join("harbor-log"), None);
    let home = base.join("home");
    let out = base.join("daily_brf");
    let snaps = base.join("snaps");
    let ex = example();
    let meta_ex = root_file("examples/meta.filled.html");

    // One full session; its outputs, each run from a clean home and output folder at the same paths.
    let session = |which: &str| -> (Vec<Out>, Vec<(String, String)>, Vec<(String, String)>) {
        reset(&home);
        let _ = std::fs::remove_dir_all(&out);
        std::fs::create_dir_all(&snaps).unwrap();
        let p = Pair { old, home: &home, extra: vec![] };
        let run = |args: &[&str], cwd: &str| if which == "node" { p.node(args, cwd) } else { p.rust(args, cwd) };
        let mut log = Vec::new();
        log.push(run(&["context"], &repo));
        log.push(run(&["init", "--output", &out], &repo));
        log.push(run(&["init", "--output", &format!("{out}-elsewhere")], &repo));
        log.push(run(&["config"], &repo));
        log.push(run(&["context"], &repo));
        log.push(run(&["build", &ex, "--out", &base.join("w/brf_tidewater_20261102.html")], &repo));
        log.push(run(&["register", "--reason", "no"], &repo));
        log.push(run(&["register", "--reason", "Please brief Tidewater", "--everyday-name", "Tidewater"], &repo));
        log.push(run(&["register", "--reason", "again please"], &repo));
        log.push(run(&["register", "--reason", "Add harbor", "--repo-name", "Harbor_Log"], &other));
        let ctx = run(&["context"], &repo);
        let ctx_json: Value = serde_json::from_str(&ctx.stdout).unwrap_or(Value::Null);
        log.push(ctx);
        log.push(run(&["context", "--meta"], &repo));
        let work = ctx_json["work_dir"].as_str().unwrap_or("/nonexistent").to_string();
        let name = ctx_json["output_name"].as_str().unwrap_or("x").to_string();
        let before = format!("{snaps}/{which}-before.json");
        log.push(run(&["snapshot", "--out", &before], &repo));
        log.push(run(&["snapshot", "--out", &format!("{repo}/inside.json")], &repo));
        let built = format!("{work}/{name}");
        log.push(run(&["build", &ex, "--out", &built], &repo));
        log.push(run(&["build", &ex, "--out", &format!("{out}/{name}")], &repo));
        log.push(run(&["build", &ex, "--out", &format!("{repo}/{name}")], &other));
        log.push(run(&["check", "--file", &built], &repo));
        log.push(run(&["check", "--file", &built, "--json"], &repo));
        log.push(run(&["snapshot", "--compare", &before], &repo));
        log.push(run(&["deliver", &built], &repo));
        log.push(run(&["deliver", &built], &repo));
        // A same-day rebrief, then the meta brief, then the folder check.
        log.push(run(&["build", &ex, "--out", &built], &repo));
        log.push(run(&["deliver", &built], &repo));
        let meta_ctx: Value = serde_json::from_str(&run(&["context", "--meta"], &repo).stdout).unwrap_or(Value::Null);
        let meta_built = format!("{}/{}", meta_ctx["work_dir"].as_str().unwrap_or("/x"), meta_ctx["output_name"].as_str().unwrap_or("x"));
        log.push(run(&["build", &meta_ex, "--out", &meta_built], &repo));
        log.push(run(&["deliver", &meta_built], &repo));
        log.push(run(&["context", "--meta"], &repo));
        log.push(run(&["context"], &repo));
        write(&format!("{out}/notes.txt"), "x");
        std::fs::create_dir_all(format!("{out}/stray")).unwrap();
        write(&format!("{out}/archive/odd-name.html"), "x");
        write(&format!("{out}/brf_stranger_20261001.html"), "<p>x</p>");
        log.push(run(&["check"], &repo));
        log.push(run(&["check", "--json"], &repo));
        // Errors and usage.
        log.push(run(&["deliver"], &repo));
        log.push(run(&["deliver", &format!("{work}/nope.html")], &repo));
        log.push(run(&["build"], &repo));
        log.push(run(&["build", &ex], &repo));
        log.push(run(&["check", "--file"], &repo));
        log.push(run(&["check", "--file", &format!("{work}/missing.html")], &repo));
        log.push(run(&["snapshot"], &repo));
        log.push(run(&["snapshot", "--compare", &format!("{snaps}/missing.json")], &repo));
        log.push(run(&["frobnicate"], &repo));
        log.push(run(&["help"], &repo));
        log.push(run(&[], &repo));
        log.push(run(&["version"], &repo));
        log.push(run(&["context"], &base.path().to_string()));
        let config = tree(&format!("{home}/cfg"), old);
        (log, tree(&out, old), config)
    };

    let (a_log, a_out, a_cfg) = session("node");
    let (b_log, b_out, b_cfg) = session("rust");
    assert_eq!(a_log.len(), b_log.len());
    for (i, (a, b)) in a_log.iter().zip(b_log.iter()).enumerate() {
        let is_folder_check = a.stdout.contains("registered project(s)");
        if is_folder_check {
            // The 2.0.0 fix: the meta brief is no longer counted as a project brief.
            let fixed = Regex::new(r"(\d+) current brief\(s\)").unwrap();
            let n: usize = fixed.captures(&a.stdout).unwrap()[1].parse().unwrap();
            let expected = fixed.replace(&a.stdout, format!("{} current project brief(s)", n - 1).as_str()).into_owned();
            assert_eq!(b.code, a.code, "step {i}: folder check exit code");
            assert_eq!(b.stdout, expected, "step {i}: folder check output");
            continue;
        }
        if a.stdout.contains("\"current_count\"") {
            let mut aj: Value = serde_json::from_str(&a.stdout).unwrap();
            let bj: Value = serde_json::from_str(&b.stdout).unwrap();
            aj["current_count"] = Value::from(aj["current_count"].as_u64().unwrap() - 1);
            assert_eq!(aj, bj, "step {i}: folder check JSON");
            continue;
        }
        assert_same(&format!("flow step {i}"), a, b);
    }
    // The session really did file briefs, archive a same-day copy and register two projects.
    let names: Vec<&str> = a_out.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.iter().any(|n| n.starts_with("/archive/brf_tidewater_") && n.contains("-v1")), "{names:?}");
    assert!(names.iter().any(|n| n.starts_with("/meta_brf_")), "{names:?}");
    assert!(a_out.iter().any(|(n, text)| n == "/registry.json" && text.contains("harbor-log") && text.contains("Please brief Tidewater")));
    assert_eq!(a_out, b_out, "the output folders differ");
    assert_eq!(a_cfg, b_cfg, "the config files differ");

    // Each version reads the other's snapshot and agrees nothing changed.
    let p = Pair { old, home: &home, extra: vec![] };
    let x = p.rust(&["snapshot", "--compare", &format!("{snaps}/node-before.json")], &repo);
    let y = p.node(&["snapshot", "--compare", &format!("{snaps}/rust-before.json")], &repo);
    assert_eq!((x.code, y.code), (0, 0), "{x:?} {y:?}");
    let node_snap: Value = serde_json::from_str(&read(&format!("{snaps}/node-before.json"))).unwrap();
    let rust_snap: Value = serde_json::from_str(&read(&format!("{snaps}/rust-before.json"))).unwrap();
    assert_eq!(node_snap["digests"], rust_snap["digests"]);
    assert_eq!(node_snap["digest"], rust_snap["digest"]);
}

#[test]
fn parity_of_snapshots_on_a_busy_repository() {
    let old = old_or_skip!();
    let home = temp_dir();
    let snaps = temp_dir();
    let parent = temp_dir();
    let repo = git_repo(&parent.join("busy"), None);
    // Staged, unstaged, untracked (including a symlink and odd names), ignored, stashes, branches, tags.
    write(&format!("{repo}/.gitignore"), "ignored/\n");
    git(&repo, &["add", ".gitignore"]);
    git(&repo, &["commit", "-q", "-m", "ignore"]);
    git(&repo, &["tag", "v1"]);
    git(&repo, &["branch", "side"]);
    write(&format!("{repo}/README.md"), "stashed\n");
    git(&repo, &["stash", "-q"]);
    write(&format!("{repo}/README.md"), "staged\n");
    git(&repo, &["add", "README.md"]);
    write(&format!("{repo}/README.md"), "staged then edited\n");
    std::fs::create_dir_all(format!("{repo}/ignored")).unwrap();
    write(&format!("{repo}/ignored/x"), "x");
    for name in ["new.txt", "\u{e9}t\u{e9}.txt", "\u{1F600}.txt", "\u{ff5e}.txt", "sub dir/a b.txt"] {
        let path = format!("{repo}/{name}");
        std::fs::create_dir_all(Path::new(&path).parent().unwrap()).unwrap();
        write(&path, name);
    }
    std::os::unix::fs::symlink("new.txt", format!("{repo}/link")).unwrap();
    let p = Pair { old, home: home.path(), extra: vec![] };
    let a = p.node(&["snapshot", "--out", &snaps.join("a.json")], &repo);
    let b = p.rust(&["snapshot", "--out", &snaps.join("b.json")], &repo);
    assert_eq!((a.code, b.code), (0, 0), "{a:?} {b:?}");
    let aj: Value = serde_json::from_str(&a.stdout).unwrap();
    let bj: Value = serde_json::from_str(&b.stdout).unwrap();
    for key in ["brf_snapshot", "repo", "head", "branch", "untracked_files", "digests", "digest"] {
        assert_eq!(aj[key], bj[key], "{key}");
    }
    assert_eq!(normalize(&a.stdout, old), normalize(&b.stdout, old));
    both(&p, "compare with the other's", &["snapshot", "--compare", &snaps.join("a.json")], &repo);
    both(&p, "compare with the other's", &["snapshot", "--compare", &snaps.join("b.json")], &repo);
    write(&format!("{repo}/new.txt"), "changed");
    both(&p, "compare after a change", &["snapshot", "--compare", &snaps.join("a.json")], &repo);
}

/// A copy of a real brief folder, checked by both. Run with
/// BRF_PARITY_REAL_DIR=<folder> cargo test --test parity -- --ignored
#[test]
#[ignore]
fn parity_on_a_copy_of_a_real_brief_folder() {
    let old = old_or_skip!();
    let Ok(real) = std::env::var("BRF_PARITY_REAL_DIR") else {
        eprintln!("set BRF_PARITY_REAL_DIR to a brief folder");
        return;
    };
    let copy = temp_dir();
    let st = Command::new("cp").args(["-R", &format!("{real}/."), copy.path()]).status().unwrap();
    assert!(st.success());
    // Only what the check reads: never the owner's own templates.
    let _ = std::fs::remove_dir_all(copy.join("templates"));
    let home = temp_dir();
    let p = Pair { old, home: home.path(), extra: vec![("BRF_OUTPUT_DIR".into(), copy.path().into())] };
    let a = p.node(&["check"], copy.path());
    let b = p.rust(&["check"], copy.path());
    assert_eq!(a.code, b.code, "{a:?}\n{b:?}");
    let fixed = Regex::new(r"(\d+) current brief\(s\)").unwrap();
    let n: usize = fixed.captures(&a.stdout).unwrap()[1].parse().unwrap();
    let meta = brf::folder::sorted_names(copy.path()).iter().any(|f| f.starts_with("meta_brf_") && f.ends_with(".html"));
    let expected = fixed.replace(&a.stdout, format!("{} current project brief(s)", n - usize::from(meta)).as_str()).into_owned();
    assert_eq!(b.stdout, expected);
    eprintln!("real folder: node said {:?}", a.stdout.lines().rev().nth(1));
    eprintln!("real folder: rust said {:?}", b.stdout.lines().rev().nth(1));
    let mut files = 0;
    for dir in [copy.path().to_string(), copy.join("archive")] {
        for name in brf::folder::sorted_names(&dir) {
            if name.ends_with(".html") {
                let f = format!("{dir}/{name}");
                both(&p, &name, &["check", "--file", &f], copy.path());
                both(&p, &name, &["check", "--file", &f, "--json"], copy.path());
                files += 1;
            }
        }
    }
    let ctx = both(&p, "meta context", &["context", "--meta"], copy.path());
    assert_eq!(ctx.code, 0);
    eprintln!("real folder: {files} briefs compared");
}
