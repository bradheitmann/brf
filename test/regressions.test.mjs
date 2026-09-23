// One test per defect found in the first independent review.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, readdirSync, readFileSync, renameSync, utimesSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { build } from "../lib/build.mjs";
import { checkFolder } from "../lib/check.mjs";
import { checkHtml } from "../lib/check.mjs";
import { writeJsonAtomic } from "../lib/config.mjs";
import { misterClean } from "../lib/context.mjs";
import { deliver, withLock } from "../lib/deliver.mjs";
import { ensureLayout, nextNumber } from "../lib/folder.mjs";
import { META_KEY } from "../lib/naming.mjs";
import { emptyRegistry, loadRegistry, matchCheckout, registryPath, remoteKey } from "../lib/registry.mjs";
import { compareSnapshot, saveSnapshot, takeSnapshot } from "../lib/snapshot.mjs";
import { cli, gitRepo, ROOT, tempDir } from "./helpers.mjs";

const EXAMPLE = join(ROOT, "examples", "tidewater.filled.html");
const git = (cwd, ...args) => execFileSync("git", ["-c", "user.email=brf-test", "-c", "user.name=t", "-c", "commit.gpgsign=false", ...args], { cwd, stdio: "ignore" });

function registered(name = "tidewater") {
  const home = tempDir();
  const repo = gitRepo(join(tempDir(), name));
  const out = join(tempDir(), "daily_brf");
  assert.equal(cli(["init", "--output", out], { cwd: repo, home }).code, 0);
  assert.equal(cli(["register", "--reason", "Please brief it"], { cwd: repo, home }).code, 0);
  return { home, repo, out };
}

test("build refuses an output or proof folder inside the repository", () => {
  const { home, repo } = registered();
  const status = execFileSync("git", ["status", "--porcelain", "--ignored"], { cwd: repo, encoding: "utf8" });
  let r = cli(["build", EXAMPLE, "--out", "brf_tidewater_20261102.html"], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /inside a repository/);
  r = cli(["build", EXAMPLE, "--out", join(tempDir(), "brf_tidewater_20261102.html"), "--proof-dir", join(repo, "proof")], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.equal(execFileSync("git", ["status", "--porcelain", "--ignored"], { cwd: repo, encoding: "utf8" }), status);
});

test("build refuses to write into the output folder", () => {
  const { home, repo, out } = registered();
  const r = cli(["build", EXAMPLE, "--out", join(out, "brf_tidewater_20261102.html")], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /inside the output folder/);
});

test("an unregistered repository with a registered name is stopped", () => {
  const { home, out } = registered("tidewater");
  const impostor = gitRepo(join(tempDir(), "tidewater"), { remote: "https://github.com/someone-else/tidewater.git" });
  const ctx = JSON.parse(cli(["context"], { cwd: impostor, home }).stdout);
  assert.equal(ctx.registered, false);
  assert.match(ctx.stop, /different checkout/);
  assert.ok(out);
});

test("a project called meta does not share numbers or filing with the meta brief", () => {
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "meta" }] });
  writeFileSync(join(out, "meta_brf_20260920.html"), '<meta name="brf-brief" content="1">');
  assert.equal(nextNumber(out, "meta"), 1);
  assert.equal(nextNumber(out, META_KEY), 2);
  const r = build({ input: EXAMPLE, out: join(tempDir(), "brf_meta_20261102.html"), outputDir: out });
  assert.equal(r.brief, 1);
  const d = deliver({ file: r.path, outputDir: out, cacheDir: tempDir() });
  assert.deepEqual(d.archived, []);
  assert.equal(readFileSync(join(out, "meta_brf_20260920.html"), "utf8"), '<meta name="brf-brief" content="1">');
});

test("context output survives a pipe at any size", () => {
  const { home, repo, out } = registered();
  for (let i = 0; i < 600; i++) {
    const d = new Date(Date.UTC(2024, 0, 1 + i));
    const stamp = `${d.getUTCFullYear()}${String(d.getUTCMonth() + 1).padStart(2, "0")}${String(d.getUTCDate()).padStart(2, "0")}`;
    writeFileSync(join(out, "archive", `brf_tidewater_${stamp}.html`), "<p>old</p>");
  }
  const r = cli(["context"], { cwd: repo, home });
  assert.equal(r.code, 0);
  assert.ok(r.stdout.length > 70000);
  assert.equal(JSON.parse(r.stdout).brief_number, 601);
});

test("the snapshot sees edits, branch switches and stash round trips", () => {
  const repo = gitRepo(join(tempDir(), "snap"));
  writeFileSync(join(repo, "README.md"), "edited once\n");
  const file = join(tempDir(), "before.json");
  const fresh = () => saveSnapshot(file, takeSnapshot(repo));
  fresh();
  writeFileSync(join(repo, "README.md"), "edited twice\n");
  assert.equal(compareSnapshot(file, repo).same, false, "further edit to a modified file");
  fresh();
  git(repo, "switch", "-q", "-c", "other");
  assert.equal(compareSnapshot(file, repo).same, false, "branch switch at the same commit");
  git(repo, "switch", "-q", "-");
  fresh();
  git(repo, "stash", "-q");
  git(repo, "stash", "apply", "-q");
  assert.equal(compareSnapshot(file, repo).same, false, "stash then apply");
  fresh();
  git(repo, "stash", "-q");
  git(repo, "stash", "pop", "-q");
  assert.equal(compareSnapshot(file, repo).same, false, "stash then pop");
  fresh();
  git(repo, "switch", "-q", "-c", "away");
  git(repo, "switch", "-q", "-");
  assert.equal(compareSnapshot(file, repo).same, false, "switch away and back");
  fresh();
  writeFileSync(join(repo, "untracked.txt"), "a");
  fresh();
  writeFileSync(join(repo, "untracked.txt"), "b");
  assert.equal(compareSnapshot(file, repo).same, false, "untracked content change");
  fresh();
  assert.throws(() => compareSnapshot(file, gitRepo(join(tempDir(), "other"))), /taken in/);
});

test("Mister Clean is found on PATH, and its version comes only from its own package", () => {
  const root = tempDir();
  const pkg = join(root, "global", "node_modules", "@bradheitmann", "mister-clean");
  mkdirSync(join(pkg, "bin"), { recursive: true });
  writeFileSync(join(pkg, "package.json"), JSON.stringify({ name: "@bradheitmann/mister-clean", version: "9.9.9" }));
  writeFileSync(join(pkg, "bin", "mister-clean.js"), "console.log('usage')\n");
  // A package-manager shim, next to an unrelated package.json that must be ignored.
  const shims = join(root, "shims");
  mkdirSync(shims);
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: "someone-else", version: "1.0.0" }));
  writeFileSync(join(shims, "mister-clean"), `#!/bin/sh\nexec node "$basedir/x.js" "$@"\n# cmd-shim-target=${join(pkg, "bin", "mister-clean.js")}\n`);
  chmodSync(join(shims, "mister-clean"), 0o755);
  assert.deepEqual(misterClean(shims), { path: join(shims, "mister-clean"), version: "9.9.9" });
  const plain = join(root, "plain");
  mkdirSync(plain);
  writeFileSync(join(plain, "mister-clean"), "#!/bin/sh\necho usage; exit 2\n");
  chmodSync(join(plain, "mister-clean"), 0o755);
  assert.deepEqual(misterClean(plain), { path: join(plain, "mister-clean"), version: null });
  assert.equal(misterClean(tempDir()), null);
});

test("check catches every way a page can run code or reach the network", () => {
  const dir = tempDir();
  const html = readFileSync(build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), example: true }).path, "utf8");
  const name = "brf_tidewater_20261102.html";
  const cases = {
    "svg image": '<svg><image href="https://example.com/a.png"/></svg>',
    srcset: '<img srcset="https://example.com/a.png 2x">',
    "meta refresh": '<meta http-equiv="refresh" content="0;url=https://example.com">',
    "event handler": '<img src="x.png" onerror="fetch(1)">',
    "javascript link": '<a href="javascript:alert(1)">x</a>',
    iframe: "<iframe></iframe>",
    "protocol-relative": '<link rel="stylesheet" href="//example.com/a.css">',
  };
  for (const [what, snippet] of Object.entries(cases)) {
    const problems = checkHtml(html.replace("</main>", `${snippet}</main>`), { name });
    assert.ok(problems.length > 0, `${what} was not caught`);
  }
  assert.deepEqual(checkHtml(html.replace("</main>", '<a href="https://example.com">a link</a></main>'), { name }), []);
});

test("a stamp that disagrees with the file name is refused", () => {
  const dir = tempDir();
  const r = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261102.html"), example: true });
  const html = readFileSync(r.path, "utf8");
  assert.match(checkHtml(html, { name: "meta_brf_20261102.html" }).join("\n"), /project brief but named as a meta brief/);
  assert.match(checkHtml(html, { name: "brf_tidewater_20261103.html" }).join("\n"), /stamped 2026-11-02 but named 2026-11-03/);
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), emptyRegistry());
  const renamed = join(dir, "meta_brf_20261102.html");
  writeFileSync(renamed, html);
  assert.throws(() => deliver({ file: renamed, outputDir: out, cacheDir: tempDir() }), /named as a meta brief/);
});

test("the whole flow leaves the repository exactly as it was", () => {
  const { home, repo, out } = registered();
  const ctx = JSON.parse(cli(["context"], { cwd: repo, home }).stdout);
  const before = join(ctx.work_dir, "before.json");
  assert.equal(cli(["snapshot", "--out", before], { cwd: repo, home }).code, 0);
  const built = join(ctx.work_dir, ctx.output_name);
  assert.equal(cli(["build", EXAMPLE, "--out", built], { cwd: repo, home }).code, 0);
  assert.equal(cli(["check", "--file", built], { cwd: repo, home }).code, 0);
  const same = JSON.parse(cli(["snapshot", "--compare", before], { cwd: repo, home }).stdout);
  assert.equal(same.same, true);
  const d = JSON.parse(cli(["deliver", built], { cwd: repo, home }).stdout);
  assert.equal(d.brief, 1);
  assert.equal(JSON.parse(cli(["snapshot", "--compare", before], { cwd: repo, home }).stdout).same, true);
  assert.ok(out);
});

test("another clone of a project registered under its own name is recognised", () => {
  const home = tempDir();
  const remote = "https://github.com/acme/Harbor_Svc.git";
  const first = gitRepo(join(tempDir(), "harbor"), { remote });
  const out = join(tempDir(), "daily_brf");
  assert.equal(cli(["init", "--output", out], { cwd: first, home }).code, 0);
  assert.equal(cli(["register", "--reason", "Add Harbor Log to my briefs", "--repo-name", "harbor-log"], { cwd: first, home }).code, 0);
  const second = gitRepo(join(tempDir(), "harbor-copy"), { remote });
  const ctx = JSON.parse(cli(["context"], { cwd: second, home }).stdout);
  assert.equal(ctx.registered, true);
  assert.equal(ctx.repo.repo_name, "harbor-log");
});

test("build and deliver refuse a different checkout that shares a registered name", () => {
  const { home, out } = registered("tidewater");
  const impostor = gitRepo(join(tempDir(), "tidewater"), { remote: "https://github.com/someone-else/tidewater.git" });
  const target = join(tempDir(), "brf_tidewater_20261102.html");
  const r = cli(["build", EXAMPLE, "--out", target], { cwd: impostor, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /not the registered "tidewater".*\(OTHER_CHECKOUT\)/);
  const made = build({ input: EXAMPLE, out: target, outputDir: out });
  const d = cli(["deliver", made.path], { cwd: impostor, home });
  assert.equal(d.code, 2);
  assert.match(d.stderr, /OTHER_CHECKOUT/);
});

test("snapshot --compare says what changed", () => {
  const home = tempDir();
  const repo = gitRepo(join(tempDir(), "compare"));
  const before = join(tempDir(), "before.json");
  assert.equal(cli(["snapshot", "--out", before], { cwd: repo, home }).code, 0);
  git(repo, "switch", "-q", "-c", "elsewhere");
  const r = cli(["snapshot", "--compare", before], { cwd: repo, home });
  assert.equal(r.code, 1);
  const out = JSON.parse(r.stdout);
  assert.equal(out.same, false);
  assert.ok(out.changed.includes("branch"));
  assert.notEqual(out.before.branch, out.after.branch);
});

test("check catches event handlers written without spaces or after a quoted >", () => {
  const html = readFileSync(build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), example: true }).path, "utf8");
  const name = "brf_tidewater_20261102.html";
  for (const snippet of [
    '<img/src="x.png"/onerror="alert(1)">',
    '<svg/onload="alert(1)"></svg>',
    '<img src="x.png"onerror="alert(1)">',
    '<img alt="a>b" src="x.png" onerror="alert(1)">',
    '<img alt="a>b" src="https://example.com/a.png">',
    "<img src=x alt=it's onerror=alert(1)>",
    '<img src=x title=a"b onerror=alert(1)//">',
    "<img alt=it's src=https://example.com/a.png>",
    '<a href="&#106;avascript:alert(1)">x</a>',
    '<img src="https:&#47;&#47;example.com/a.png">',
    '<a href="java\tscript:alert(1)">x</a>',
  ]) {
    assert.ok(checkHtml(html.replace("</main>", `${snippet}</main>`), { name }).length > 0, snippet);
  }
  assert.deepEqual(checkHtml(html.replace("</main>", '<p title="turn online=yes">text</p></main>'), { name }), []);
  assert.deepEqual(checkHtml(html.replace("</main>", '<p title="set src=https://example.com">text</p></main>'), { name }), []);
});

test("remote keys keep the whole path, so unrelated repos with the same last two parts differ", () => {
  const out = tempDir();
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "api", remote: "https://dev.azure.com/orgA/Payments/_git/api" }] });
  const reg = loadRegistry(out);
  const other = matchCheckout(reg, { repo_name: "api", main_path: tempDir(), remote: "https://dev.azure.com/orgB/Unrelated/_git/api" });
  assert.equal(other.project, null);
  assert.equal(matchCheckout(reg, { repo_name: "api", main_path: tempDir(), remote: `https://user${"@"}dev.azure.com/orgA/Payments/_git/api/` }).project.repo_name, "api");
  assert.notEqual(remoteKey("https://gitlab.com/team-a/platform/api.git"), remoteKey("https://gitlab.com/team-b/platform/api.git"));
});

test("the snapshot ignores signature display, even on signed commits", (t) => {
  if (spawnSync("ssh-keygen", ["-V"], { stdio: "ignore" }).error) return t.skip("ssh-keygen not available");
  const keys = tempDir();
  const key = join(keys, "id");
  execFileSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-f", key]);
  const repo = gitRepo(join(tempDir(), "signed"));
  git(repo, "config", "gpg.format", "ssh");
  git(repo, "config", "user.signingkey", key);
  writeFileSync(join(repo, "signed.txt"), "x");
  git(repo, "add", "signed.txt");
  execFileSync("git", ["-c", "user.email=brf-test", "-c", "user.name=t", "-c", "commit.gpgsign=true", "commit", "-q", "-m", "signed"], { cwd: repo, stdio: "ignore" });
  git(repo, "config", "log.showSignature", "true");
  const file = join(tempDir(), "before.json");
  saveSnapshot(file, takeSnapshot(repo));
  // Trusting the key changes what signature verification prints; the repository is unchanged.
  writeFileSync(join(keys, "allowed"), `brf-test ${readFileSync(`${key}.pub`, "utf8")}`);
  git(repo, "config", "gpg.ssh.allowedSignersFile", join(keys, "allowed"));
  const r = compareSnapshot(file, repo);
  assert.equal(r.same, true, `changed: ${r.changed}`);
});

test("check follows HTML's rules for comments, text-only elements, whitespace and entities", () => {
  const html = readFileSync(build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), example: true }).path, "utf8");
  const name = "brf_tidewater_20261102.html";
  const nbsp = "\u00a0";
  for (const snippet of [
    "<!--> <img src=x onerror=alert(1)> -->",
    "<!---> <img src=x onerror=alert(1)> -->",
    "<!-- a --!> <img src=x onerror=alert(1)> -->",
    '<img alt="<!--" src=x onerror=alert(1)><p title="-->">',
    '<style>/* <a title=" */</style><img src=x onerror=alert(1)><style>/* " */</style>',
    '<!x <a title=" ><img src=x onerror=alert(1)><b title=x">',
    `<p title=${nbsp}"a onerror=alert(1) b=x">x</p>`,
    '<p class="https&colon;//example.com">x</p><img src="https&colon;//example.com/a.png">',
    '<a href="https&colon;//example.com">x</a>',
    '<link rel=preload as=image imagesrcset="https://example.com/a.png">',
    "<style>body{background:url(\\68ttps://example.com/a.png)}</style>",
    "<style>body{background:url(https:example.com/a.png)}</style>",
    '<svg><style>rect{fill:url(&#104;ttps://example.com/a)}</style></svg>',
    '<p style="background:url(https://example.com/a.png)">x</p>',
    '<meta http-equiv="refresh" content="0;url=https://example.com">',
  ]) {
    assert.ok(checkHtml(html.replace("</main>", `${snippet}</main>`), { name }).length > 0, snippet);
  }
  const at = "@";
  for (const ordinary of [
    "<p>The fonts came in with @import once; now they are embedded.</p>",
    `<p>Write to billing${at}importfreight.example for invoices.</p>`,
    "<p>If a &lt; b and x&lt;y, nothing happens &#9999999; at all.</p>",
    '<p title="set src=https://example.com">x</p>',
    '<a href="https://example.com/?a=1&amp;b=2">a link</a>',
    '<a href="&#104ttps://example.com/&#9999999;">a link written with odd entities</a>',
  ]) {
    assert.deepEqual(checkHtml(html.replace("</main>", `${ordinary}</main>`), { name }), [], ordinary);
  }
});

test("remote keys agree across HTTPS and SSH on Azure DevOps and Bitbucket Server", () => {
  const at = "@";
  assert.equal(remoteKey("https://dev.azure.com/OrgA/Payments/_git/api"), remoteKey(`git${at}ssh.dev.azure.com:v3/OrgA/Payments/api`));
  assert.equal(remoteKey("https://bitbucket.example.com/scm/proj/repo.git"), remoteKey(`ssh://git${at}bitbucket.example.com:7999/proj/repo.git`));
  assert.equal(remoteKey("https://github.com/scm/repo"), "scm/repo");
});

test("CSS comment markers inside strings cannot hide a fetch", () => {
  const html = readFileSync(build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), example: true }).path, "utf8");
  for (const css of [
    '.a{content:"/*"}body{background:url(https://example.com/a.png)}.b{content:"*/"}',
    ".a{b:\\/*}body{background:url(https://example.com/a.png)}.c{d:*/}",
  ]) {
    const problems = checkHtml(html.replace("</main>", `<style>${css}</style></main>`), { name: "brf_tidewater_20261102.html" });
    assert.match(problems.join("\n"), /fetches from the network/, css);
  }
});

test("build and deliver refuse a page with a script, whether or not check was run", () => {
  const dir = tempDir();
  const filled = join(dir, "bad.filled.html");
  writeFileSync(filled, readFileSync(EXAMPLE, "utf8").replace("</main>", '<script>fetch("https://example.com/x")</script></main>'));
  assert.throws(() => build({ input: filled, out: join(dir, "brf_tidewater_20261102.html"), example: true }), /UNSAFE|script element/);
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }] });
  const good = build({ input: EXAMPLE, out: join(dir, "brf_tidewater_20261103.html"), outputDir: out });
  writeFileSync(good.path, readFileSync(good.path, "utf8").replace("</main>", "<script>1</script></main>"));
  assert.throws(() => deliver({ file: good.path, outputDir: out, cacheDir: tempDir() }), /script element/);
});

test("build refuses a registered project's folder from anywhere, and another spelling of it", () => {
  const { home, repo } = registered();
  const elsewhere = tempDir();
  let r = cli(["build", EXAMPLE, "--out", join(repo, "brf_tidewater_20261102.html")], { cwd: elsewhere, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /inside a repository/);
  const upper = repo.replace(/tidewater$/, "TIDEWATER");
  const caseInsensitive = existsSync(upper);
  if (caseInsensitive) {
    r = cli(["build", EXAMPLE, "--out", join(upper, "brf_tidewater_20261102.html")], { cwd: repo, home });
    assert.equal(r.code, 2, "a differently cased path to the repository");
  }
  r = cli(["build", EXAMPLE, `--out=${join(tempDir(), "brf_tidewater_20261102.html")}`], { cwd: repo, home });
  assert.equal(r.code, 0, "--flag=value works");
});

test("a missing output folder is the owner's to fix", () => {
  const { home, repo, out } = registered();
  renameSync(out, `${out}-away`);
  const r = cli(["context"], { cwd: repo, home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /Tell the owner; only the owner runs brf init/);
  assert.equal(existsSync(out), false);
});

test("the same folder with a different remote is a different project", () => {
  const out = tempDir();
  const folder = tempDir();
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "api", local_path: folder, remote: "https://github.com/acme/api.git" }] });
  const r = matchCheckout(loadRegistry(out), { repo_name: "website", main_path: folder, remote: "https://github.com/someone-else/website.git" });
  assert.equal(r.project, null);
  assert.equal(r.conflict.repo_name, "api");
  assert.equal(matchCheckout(loadRegistry(out), { repo_name: "x", main_path: folder, remote: null }).project.repo_name, "api");
});

test("deliver checks the registry itself", () => {
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }] });
  const r = build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), outputDir: out });
  writeJsonAtomic(registryPath(out), emptyRegistry());
  assert.throws(() => deliver({ file: r.path, outputDir: out, cacheDir: tempDir() }), /not in the registry/);
});

test("init on a second computer keeps the existing registry", () => {
  const { out } = registered();
  const before = readFileSync(join(out, "registry.json"), "utf8");
  const other = tempDir();
  assert.equal(cli(["init", "--output", out], { cwd: other, home: tempDir() }).code, 0);
  assert.equal(readFileSync(join(out, "registry.json"), "utf8"), before);
});

test("check notices a current brief whose number is behind the archive", () => {
  const out = tempDir();
  ensureLayout(out);
  writeJsonAtomic(registryPath(out), { ...emptyRegistry(), projects: [{ repo_name: "tidewater" }] });
  const r = build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), outputDir: out, brief: 1 });
  writeFileSync(join(out, "brf_tidewater_20261102.html"), readFileSync(r.path));
  writeFileSync(join(out, "archive", "brf_tidewater_20261001.html"), '<meta name="brf-brief" content="5">');
  assert.match(checkFolder(out, { today: "20261102" }).problems.join("\n"), /is brief 1 but the archive holds brief 5/);
});

test("the snapshot sees a change that is both staged and in the working file", () => {
  const repo = gitRepo(join(tempDir(), "staged"));
  const file = join(tempDir(), "before.json");
  saveSnapshot(file, takeSnapshot(repo));
  writeFileSync(join(repo, "README.md"), "changed and staged\n");
  git(repo, "add", "README.md");
  const r = compareSnapshot(file, repo);
  assert.equal(r.same, false);
  assert.ok(r.changed.includes("staged"));
});

test("deliveries take turns through the lock, and a stale lock is broken", () => {
  const cache = tempDir();
  const out = tempDir();
  let inner = null;
  withLock(cache, out, () => {
    inner = (() => { try { withLock(cache, out, () => 1, { waitMs: 300 }); return "entered"; } catch (e) { return e.code; } })();
  });
  assert.equal(inner, "LOCKED");
  assert.equal(withLock(cache, out, () => "free again"), "free again");
  // A lock left behind by a crashed process is taken over once it is old enough.
  withLock(cache, out, () => {
    const locks = join(cache, "locks");
    for (const f of readdirSync(locks)) utimesSync(join(locks, f), new Date(0), new Date(0));
    assert.equal(withLock(cache, out, () => "took over", { waitMs: 300, staleMs: 1000 }), "took over");
  });
});

test("check --file on a missing file is a usage error", () => {
  const r = cli(["check", "--file", join(tempDir(), "nope.html")], { cwd: ROOT, home: tempDir() });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /Not found/);
});

test("SVG attributes cannot spell url() with entities or CSS escapes", () => {
  const html = readFileSync(build({ input: EXAMPLE, out: join(tempDir(), "brf_tidewater_20261102.html"), example: true }).path, "utf8");
  const name = "brf_tidewater_20261102.html";
  for (const svg of [
    '<svg><rect fill="url&lpar;https://example.com/p.svg#g&rpar;"/></svg>',
    '<svg><rect fill="u\\72l(https://example.com/p.svg#g)"/></svg>',
    '<svg><rect clip-path="url (https://example.com/c.svg#c)"/></svg>',
    '<svg><rect stroke="url(https://example.com/s.svg#s)"/></svg>',
    '<svg><rect fill="u\\72\r\nl(https://example.com/p.svg#g)"/></svg>',
    '<svg><rect fill="u\\72&#13;&#10;l(https://example.com/p.svg#g)"/></svg>',
    '<style>body{background:u\\72\r\nl(https://example.com/p.gif)}</style>',
    '<style>@im\\70\r\nort "x.css";</style>',
  ]) {
    assert.ok(checkHtml(html.replace("</main>", `${svg}</main>`), { name }).length > 0, svg);
  }
  assert.deepEqual(checkHtml(html.replace("</main>", '<svg viewBox="0 0 10 10"><defs><lineargradient id="g"></lineargradient></defs><rect fill="url(#g)" transform="translate(1 2)"/></svg></main>'), { name }), []);
  assert.deepEqual(checkHtml(html.replace("</main>", '<p title="R&D and Q&A">x</p></main>'), { name }), []);
});

test("an example build still refuses a registered project's folder", () => {
  const { home, repo } = registered();
  const r = cli(["build", EXAMPLE, "--example", "--out", join(repo, "brf_tidewater_20261102.html")], { cwd: tempDir(), home });
  assert.equal(r.code, 2);
  assert.match(r.stderr, /inside a repository/);
});
